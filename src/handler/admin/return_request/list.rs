use std::collections::HashMap;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::{Order, ReturnRequest, ReturnItem};

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | requested | approved | rejected | in_transit | received | refunded
    status: Option<String>,
    q: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<ReturnRequest>("return_request");

    let mut filter = doc! {};
    if let Some(status) = &query.status {
        let variant = match status.as_str() {
            "requested" => "Requested",
            "approved" => "Approved",
            "rejected" => "Rejected",
            "in_transit" => "InTransit",
            "received" => "Received",
            "refunded" => "Refunded",
            _ => "",
        };
        if !variant.is_empty() {
            filter.insert("status", variant);
        }
    }

    // The console searches by order number, which lives on `order`, so it has
    // to be resolved to ids first.
    if let Some(q) = &query.q {
        if !q.trim().is_empty() {
            let mut order_cursor = db
                .collection::<Order>("order")
                .find(doc! { "number": { "$regex": q, "$options": "i" } })
                .await
                .expect("order lookup");
            let mut ids = Vec::new();
            while let Some(result) = order_cursor.next().await {
                match result {
                    Ok(o) => ids.push(o.uuid),
                    Err(error) => log::error!("skipping unreadable order: {:?}", error),
                }
            }
            filter.insert("order_id", doc! { "$in": ids });
        }
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);
    let result = col
        .find(filter)
        .sort(doc! { "requested_at": -1 })
        .skip((limit * (page - 1)) as u64)
        .limit(limit)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut returns = Vec::new();
    while let Some(result) = cursor.next().await {
        match result {
            Ok(r) => returns.push(r),
            Err(error) => log::error!("skipping unreadable return_request: {:?}", error),
        }
    }

    // One `$in` per page for the orders and one for the lines, rather than two
    // round trips per row.
    let order_ids: Vec<String> = returns.iter().map(|r| r.order_id.clone()).collect();
    let orders: HashMap<String, Order> = if order_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db.collection::<Order>("order").find(doc! { "uuid": { "$in": &order_ids } }).await.expect("order lookup");
        let mut map = HashMap::new();
        while let Some(result) = cursor.next().await {
            let o = match result {
                Ok(o) => o,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            map.insert(o.uuid.clone(), o);
        }
        map
    };

    let request_ids: Vec<String> = returns.iter().map(|r| r.uuid.clone()).collect();
    let mut lines: HashMap<String, Vec<ReturnItem>> = HashMap::new();
    if !request_ids.is_empty() {
        let mut cursor = db
            .collection::<ReturnItem>("return_item")
            .find(doc! { "return_request_id": { "$in": &request_ids } })
            .await
            .expect("return item lookup");
        while let Some(result) = cursor.next().await {
            let i = match result {
                Ok(i) => i,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            lines.entry(i.return_request_id.clone()).or_default().push(i);
        }
    }

    let mut items = Vec::new();
    for r in &returns {
        let order = orders.get(&r.order_id);
        let request_lines = lines.get(&r.uuid).cloned().unwrap_or_default();
        items.push(json!({
            "uuid": r.uuid,
            "order_id": r.order_id,
            "order_number": order.map(|o| o.number.clone()),
            "customer": order.map(|o| o.shipping_address.recipient_name.clone()),
            "user_id": r.user_id,
            "reason": r.reason,
            "details": r.details,
            "status": r.status.to_string(),
            "refund_amount": r.refund_amount,
            "unit_count": request_lines.iter().map(|l| l.quantity).sum::<u32>(),
            "items": request_lines,
            "requested_at": r.requested_at,
            "resolved_at": r.resolved_at,
        }));
    }

    let open = col
        .count_documents(doc! { "status": { "$in": ["Requested", "Approved", "InTransit", "Received"] } })
        .await
        .unwrap_or(0);

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
        "stats": { "open": open },
    })))
}
