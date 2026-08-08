use futures::StreamExt;
use serde_json::{json, Value};
use mongodb::bson::doc;
use crate::BuiltIns::mongo::MongoDB;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::Model::Account::{AdminAccount, AccountRole};

/// List the administrator accounts (for the Settings page).
pub async fn list(req: HttpRequest) -> Result<HttpResponse, Error> {
    let me = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let cursor = db
        .collection::<AdminAccount>("admin_account")
        .find(doc!{})
        .sort(doc!{ "created_at": 1 })
        .await;

    let mut admins: Vec<Value> = Vec::new();
    if let Ok(mut cursor) = cursor {
        while let Some(Ok(a)) = cursor.next().await {
            admins.push(json!({
                "user_id": a.uuid.clone(),
                "name": if a.name.is_empty() { "Admin".to_string() } else { a.name.clone() },
                "email": a.email_address,
                "created_at": a.created_at,
                "is_you": a.uuid == me.user_id,
            }));
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "admins": admins })))
}
