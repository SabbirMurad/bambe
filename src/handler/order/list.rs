use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Order::Order;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | processing | shipped | delivered | cancelled
    status: Option<String>,
    q: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

fn status_filter(tab: &str) -> Option<mongodb::bson::Document> {
    match tab {
        "processing" => Some(doc! { "fulfilment_status": { "$in": ["Pending", "Packing", "OnHold"] } }),
        "shipped" => Some(doc! { "fulfilment_status": "Shipped" }),
        "delivered" => Some(doc! { "fulfilment_status": "Delivered" }),
        "cancelled" => Some(doc! { "fulfilment_status": { "$in": ["Cancelled", "Returned"] } }),
        _ => None,
    }
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<Order>("order");

    let mut filter = doc! { "user_id": &user.user_id };
    if let Some(tab) = query.status.as_deref() {
        if let Some(extra) = status_filter(&tab.to_lowercase()) {
            filter.extend(extra);
        }
    }
    if let Some(q) = &query.q {
        filter.insert("number", doc! { "$regex": q, "$options": "i" });
    }

    let limit = query.limit.unwrap_or(10).clamp(1, 50) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);

    let result = col
        .find(filter)
        .sort(doc! { "placed_at": -1 })
        .skip((limit * (page - 1)) as u64)
        .limit(limit)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut orders = Vec::new();
    while let Some(Ok(o)) = cursor.next().await {
        orders.push(o);
    }

    let base = doc! { "user_id": &user.user_id };
    let mut processing_filter = base.clone();
    processing_filter.extend(status_filter("processing").unwrap());
    let mut shipped_filter = base.clone();
    shipped_filter.extend(status_filter("shipped").unwrap());
    let mut delivered_filter = base.clone();
    delivered_filter.extend(status_filter("delivered").unwrap());
    let mut cancelled_filter = base.clone();
    cancelled_filter.extend(status_filter("cancelled").unwrap());

    let counts = json!({
        "all": col.count_documents(base.clone()).await.unwrap_or(0),
        "processing": col.count_documents(processing_filter).await.unwrap_or(0),
        "shipped": col.count_documents(shipped_filter).await.unwrap_or(0),
        "delivered": col.count_documents(delivered_filter).await.unwrap_or(0),
        "cancelled": col.count_documents(cancelled_filter).await.unwrap_or(0),
    });

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": orders,
        "total": total,
        "page": page,
        "limit": limit,
        "counts": counts,
    })))
}
