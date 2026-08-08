use serde_json::json;
use mongodb::bson::doc;
use crate::BuiltIns::mongo::MongoDB;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::Model::Account::{AdminAccount, AccountRole};

/// The currently signed-in administrator (for the top bar / sidebar identity).
pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let me = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let admin = db
        .collection::<AdminAccount>("admin_account")
        .find_one(doc!{ "uuid": &me.user_id })
        .await
        .ok()
        .flatten();

    let name = admin
        .as_ref()
        .map(|a| a.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Admin".to_string());

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "user_id": me.user_id,
        "name": name,
        "email": admin.as_ref().map(|a| a.email_address.clone()),
    })))
}
