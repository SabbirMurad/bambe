use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use mongodb::bson::doc;
use mongodb::Database;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Store::AdminAuditLog;
use crate::model::Account::AdminAccount;

pub async fn admin_name(db: &Database, admin_id: &str) -> String {
    db.collection::<AdminAccount>("admin_account")
        .find_one(doc! { "uuid": admin_id })
        .await
        .ok()
        .flatten()
        .map(|a| if a.name.is_empty() { "Admin".to_string() } else { a.name })
        .unwrap_or_else(|| "Admin".to_string())
}

/// Writes one line to the audit trail. `changes` is a caller-formatted
/// summary ("price 4499 -> 2999"), not a full before/after document — a
/// large product dumped twice buries the one field that actually moved.
pub async fn log(db: &Database, admin_id: &str, admin_name: &str, action: &str, target_collection: &str, target_id: &str, changes: Option<String>) {
    let entry = AdminAuditLog {
        uuid: Uuid::now_v7().to_string(),
        admin_id: admin_id.to_string(),
        admin_name: admin_name.to_string(),
        action: action.to_string(),
        target_collection: target_collection.to_string(),
        target_id: target_id.to_string(),
        changes,
        ip_address: None,
        created_at: Utc::now().timestamp_millis(),
    };
    db.collection::<AdminAuditLog>("admin_audit_log").insert_one(&entry).await.ok();
}

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    admin_id: Option<String>,
    target_collection: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<AdminAuditLog>("admin_audit_log");

    let mut filter = doc! {};
    if let Some(id) = &query.admin_id { filter.insert("admin_id", id); }
    if let Some(c) = &query.target_collection { filter.insert("target_collection", c); }

    let limit = query.limit.unwrap_or(50).clamp(1, 200) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);
    let result = col.find(filter).sort(doc! { "created_at": -1 }).skip((limit * (page - 1)) as u64).limit(limit).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut entries = Vec::new();
    while let Some(Ok(e)) = cursor.next().await {
        entries.push(e);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(serde_json::json!({
        "items": entries,
        "total": total,
        "page": page,
        "limit": limit,
    })))
}
