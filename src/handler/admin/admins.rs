use futures::StreamExt;
use serde_json::{json, Value};
use mongodb::bson::doc;
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::model::Account::AdminAccount;

/// List the administrator accounts (for the Settings page).
pub async fn list(req: HttpRequest) -> Result<HttpResponse, Error> {
    let me = require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();

    let cursor = db
        .collection::<AdminAccount>("admin_account")
        .find(doc!{})
        .sort(doc!{ "created_at": 1 })
        .await;

    let mut admins: Vec<Value> = Vec::new();
    if let Ok(mut cursor) = cursor {
        while let Some(result) = cursor.next().await {
            let a = match result {
                Ok(a) => a,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
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
