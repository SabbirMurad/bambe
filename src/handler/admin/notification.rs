use chrono::Utc;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::AdminNotification::AdminNotification;

const COLLECTION: &str = "admin_notification";

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    limit: Option<u32>,
}

/* The bell's feed: newest first, plus how many are still unread. */
pub async fn list(req: HttpRequest, query: web::Query<ListQuery>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let col = db.collection::<AdminNotification>(COLLECTION);

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let mut items = Vec::new();
    if let Ok(mut cursor) = col.find(doc! {}).sort(doc! { "created_at": -1 }).limit(limit).await {
        while let Some(result) = cursor.next().await {
            match result {
                Ok(n) => items.push(n),
                Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
            }
        }
    }
    let unread = col.count_documents(doc! { "read_at": null }).await.unwrap_or(0);

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "items": items, "unread": unread })))
}

pub async fn mark_read(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    db.collection::<AdminNotification>(COLLECTION)
        .update_one(
            doc! { "uuid": path.into_inner(), "read_at": null },
            doc! { "$set": { "read_at": Utc::now().timestamp_millis() } },
        )
        .await
        .ok();
    Ok(Response::ok_message("Marked read"))
}

pub async fn mark_all_read(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    db.collection::<AdminNotification>(COLLECTION)
        .update_many(doc! { "read_at": null }, doc! { "$set": { "read_at": Utc::now().timestamp_millis() } })
        .await
        .ok();
    Ok(Response::ok_message("All marked read"))
}
