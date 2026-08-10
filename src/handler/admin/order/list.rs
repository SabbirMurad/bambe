use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::Order;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | pending | packing | shipped | delivered | cancelled | on_hold | returned
    status: Option<String>,
    payment_status: Option<String>,
    q: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    page: Option<u32>,
    limit: Option<u32>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Order>("order");

    let mut filter = doc! {};
    if let Some(status) = &query.status {
        let variant = match status.as_str() {
            "pending" => "Pending",
            "packing" => "Packing",
            "shipped" => "Shipped",
            "delivered" => "Delivered",
            "cancelled" => "Cancelled",
            "on_hold" => "OnHold",
            "returned" => "Returned",
            _ => "",
        };
        if !variant.is_empty() {
            filter.insert("fulfilment_status", variant);
        }
    }
    if let Some(payment_status) = &query.payment_status {
        filter.insert("payment_status", payment_status);
    }
    if let Some(q) = &query.q {
        filter.insert("$or", vec![
            doc! { "number": { "$regex": q, "$options": "i" } },
            doc! { "shipping_address.recipient_name": { "$regex": q, "$options": "i" } },
        ]);
    }
    if query.from.is_some() || query.to.is_some() {
        let mut range = doc! {};
        if let Some(from) = query.from { range.insert("$gte", from); }
        if let Some(to) = query.to { range.insert("$lte", to); }
        filter.insert("placed_at", range);
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);
    let result = col.find(filter).sort(doc! { "placed_at": -1 }).skip((limit * (page - 1)) as u64).limit(limit).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut orders = Vec::new();
    while let Some(Ok(o)) = cursor.next().await {
        orders.push(o);
    }

    let to_fulfil = col.count_documents(doc! { "fulfilment_status": { "$in": ["Pending", "Packing"] } }).await.unwrap_or(0);
    let in_transit = col.count_documents(doc! { "fulfilment_status": "Shipped" }).await.unwrap_or(0);
    let returns = db
        .collection::<crate::model::Order::ReturnRequest>("return_request")
        .count_documents(doc! { "status": { "$in": ["Requested", "Approved", "InTransit"] } })
        .await
        .unwrap_or(0);

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": orders,
        "total": total,
        "page": page,
        "limit": limit,
        "stats": { "to_fulfil": to_fulfil, "in_transit": in_transit, "returns": returns },
    })))
}
