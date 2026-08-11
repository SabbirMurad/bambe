use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Cart::RecentlyViewed;

// Keep only the newest CAP rows per user — this collection grows faster
// than anything else in the system and nobody browses their own tail.
const CAP: u64 = 50;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    product_id: String,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<RecentlyViewed>("recently_viewed");

    let result = col
        .update_one(
            doc! { "user_id": &user.user_id, "product_id": &body.product_id },
            doc! {
                "$set": { "viewed_at": Utc::now().timestamp_millis() },
                "$setOnInsert": { "uuid": Uuid::now_v7().to_string() },
            },
        )
        .upsert(true)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = col
        .find(doc! { "user_id": &user.user_id })
        .sort(doc! { "viewed_at": -1 })
        .skip(CAP)
        .await
        .expect("recently viewed prune lookup");

    let mut stale_ids = Vec::new();
    while let Some(Ok(r)) = cursor.next().await {
        stale_ids.push(r.uuid);
    }

    if !stale_ids.is_empty() {
        col.delete_many(doc! { "uuid": { "$in": stale_ids } }).await.ok();
    }

    Ok(Response::ok_message("Recorded"))
}
