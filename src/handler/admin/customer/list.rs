use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // vip | repeat | new | dormant — omitted means every segment.
    segment: Option<String>,
    location: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

// Every account is a customer even before their first order — the
// commerce rollup just defaults to zero until `customer_profile` gets a
// row for them, so the join fills those defaults in rather than hiding
// accounts that haven't bought anything yet.
fn base_pipeline() -> Vec<Document> {
    vec![
        doc! { "$match": { "role": "User" } },
        doc! { "$lookup": {
            "from": "customer_profile",
            "localField": "uuid",
            "foreignField": "user_id",
            "as": "profile",
        } },
        doc! { "$unwind": { "path": "$profile", "preserveNullAndEmptyArrays": true } },
        doc! { "$addFields": {
            "segment": { "$ifNull": ["$profile.segment", "New"] },
            "order_count": { "$ifNull": ["$profile.order_count", 0] },
            "lifetime_spend": { "$ifNull": ["$profile.lifetime_spend", 0] },
            "location": { "$ifNull": ["$profile.location", null] },
            "last_order_at": { "$ifNull": ["$profile.last_order_at", null] },
            "loyalty_points": { "$ifNull": ["$profile.loyalty_points", 0] },
            "tags": { "$ifNull": ["$profile.tags", []] },
        } },
    ]
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let mut pipeline = base_pipeline();
    if let Some(segment) = &query.segment {
        let variant = match segment.to_lowercase().as_str() {
            "vip" => "Vip",
            "repeat" => "Repeat",
            "dormant" => "Dormant",
            _ => "New",
        };
        pipeline.push(doc! { "$match": { "segment": variant } });
    }
    if let Some(location) = &query.location {
        pipeline.push(doc! { "$match": { "location": { "$regex": location, "$options": "i" } } });
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let mut count_pipeline = pipeline.clone();
    count_pipeline.push(doc! { "$count": "total" });
    let total: u64 = match db.collection::<Document>("account_core").aggregate(count_pipeline).await {
        Ok(mut cursor) => cursor.next().await.and_then(|r| r.ok()).and_then(|d| d.get_i32("total").ok().map(|v| v as u64)).unwrap_or(0),
        Err(_) => 0,
    };

    pipeline.push(doc! { "$sort": { "order_count": -1 } });
    pipeline.push(doc! { "$skip": limit * (page - 1) });
    pipeline.push(doc! { "$limit": limit });
    pipeline.push(doc! { "$project": {
        "user_id": "$uuid",
        "email": "$email_address",
        "username": 1,
        "segment": 1,
        "order_count": 1,
        "lifetime_spend": 1,
        "location": 1,
        "last_order_at": 1,
        "loyalty_points": 1,
        "tags": 1,
        "created_at": 1,
        "_id": 0,
    } });

    let result = db.collection::<Document>("account_core").aggregate(pipeline).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut customers = Vec::new();
    while let Some(Ok(d)) = cursor.next().await {
        customers.push(d);
    }

    // Overall figures for the stat tiles — unaffected by the segment/
    // location filters above, same as the order list's stat tiles.
    let mut stats_pipeline = base_pipeline();
    stats_pipeline.push(doc! { "$group": {
        "_id": null,
        "total": { "$sum": 1 },
        "repeat_count": { "$sum": { "$cond": [{ "$gt": ["$order_count", 1] }, 1, 0] } },
        "vip_count": { "$sum": { "$cond": [{ "$eq": ["$segment", "Vip"] }, 1, 0] } },
        "avg_ltv": { "$avg": "$lifetime_spend" },
    } });
    let stats = match db.collection::<Document>("account_core").aggregate(stats_pipeline).await {
        Ok(mut cursor) => cursor.next().await.and_then(|r| r.ok()),
        Err(_) => None,
    };

    let (total_customers, repeat_count, vip_count, avg_ltv) = match &stats {
        Some(d) => (
            d.get_i32("total").unwrap_or(0),
            d.get_i32("repeat_count").unwrap_or(0),
            d.get_i32("vip_count").unwrap_or(0),
            d.get_f64("avg_ltv").unwrap_or(0.0),
        ),
        None => (0, 0, 0, 0.0),
    };
    let repeat_rate = if total_customers > 0 { repeat_count as f64 / total_customers as f64 * 100.0 } else { 0.0 };

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": customers,
        "total": total,
        "page": page,
        "limit": limit,
        "stats": {
            "total_customers": total_customers,
            "repeat_rate": repeat_rate,
            "avg_lifetime_value": avg_ltv,
            "vip_count": vip_count,
        },
    })))
}
