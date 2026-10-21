use chrono::Utc;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Coupon::Coupon;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | active | scheduled | expired | used_up | disabled
    tab: Option<String>,
    // discount_type: Percentage | Fixed | FreeShipping
    r#type: Option<String>,
    sort: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

// None of these are stored — the admin list derives Active / Scheduled /
// Expired / Used up / Disabled from the switch, the dates and the
// redemption count, same as the model comment on `Coupon` says to.
fn tab_filter(tab: &str, now: i64) -> mongodb::bson::Document {
    match tab {
        "disabled" => doc! { "enabled": false },
        "expired" => doc! { "enabled": true, "ends_at": { "$lt": now } },
        "scheduled" => doc! { "enabled": true, "starts_at": { "$gt": now } },
        "used_up" => doc! {
            "enabled": true,
            "$expr": { "$and": [
                { "$ne": ["$total_limit", null] },
                { "$gte": ["$used_count", "$total_limit"] },
            ] },
        },
        "active" => doc! {
            "enabled": true,
            "starts_at": { "$lte": now },
            "$and": [
                { "$or": [{ "ends_at": null }, { "ends_at": { "$gte": now } }] },
                { "$expr": { "$or": [
                    { "$eq": ["$total_limit", null] },
                    { "$lt": ["$used_count", "$total_limit"] },
                ] } },
            ],
        },
        _ => doc! {},
    }
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Coupon>("coupon");
    let now = Utc::now().timestamp_millis();

    let mut filter = tab_filter(query.tab.as_deref().unwrap_or("all"), now);
    if let Some(t) = &query.r#type {
        filter.insert("discount_type", t);
    }

    let sort = match query.sort.as_deref() {
        Some("oldest") => doc! { "created_at": 1 },
        Some("most_used") => doc! { "used_count": -1 },
        Some("value") => doc! { "value": -1 },
        _ => doc! { "created_at": -1 },
    };

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);
    let result = col.find(filter).sort(sort).skip((limit * (page - 1)) as u64).limit(limit).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut coupons = Vec::new();
    while let Some(result) = cursor.next().await {
        let c = match result {
            Ok(c) => c,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        coupons.push(c);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": coupons,
        "total": total,
        "page": page,
        "limit": limit,
    })))
}
