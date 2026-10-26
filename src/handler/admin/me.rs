use serde_json::json;
use mongodb::bson::doc;
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::model::Account::AdminAccount;

/// The currently signed-in administrator (for the top bar / sidebar identity).
pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let me = require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let staff = crate::Handler::Admin::Access::staff_member(&db, &me.user_id).await?;

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
        // The console hides what the role can't use; the server refuses it
        // regardless (access.rs).
        "role": staff.role,
        "role_label": crate::Handler::Admin::Access::role_label(&staff.role),
        "permissions": crate::Handler::Admin::Access::granted(&staff.role),
    })))
}
