use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cents;
use crate::model::Order::PackagingOption;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: String,
    #[serde(default)]
    description: String,
    price: Cents,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    position: u16,
    #[serde(default)]
    image: String,
    #[serde(default)]
    multi_item_only: bool,
}
fn default_true() -> bool { true }

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Shipping).await?;
    let db = MongoDB.connect();

    if body.name.trim().is_empty() {
        return Ok(Response::bad_request("Name is required"));
    }
    if body.price < 0 {
        return Ok(Response::bad_request("Price can't be negative"));
    }

    let option = PackagingOption {
        uuid: Uuid::now_v7().to_string(),
        name: body.name.trim().to_string(),
        description: body.description.trim().to_string(),
        price: body.price,
        enabled: body.enabled,
        position: body.position,
        image: body.image.trim().to_string(),
        multi_item_only: body.multi_item_only,
    };

    if let Err(error) = db.collection::<PackagingOption>("packaging_option").insert_one(&option).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "packaging_option.created", "packaging_option", &option.uuid,
        Some(option.name.clone()),
    ).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(option))
}
