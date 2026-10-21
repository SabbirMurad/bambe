use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Order::PackagingOption;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: Option<String>,
    description: Option<String>,
    price: Option<Cents>,
    enabled: Option<bool>,
    position: Option<u16>,
    image: Option<String>,
    multi_item_only: Option<bool>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<PackagingOption>("packaging_option");
    let id = path.into_inner();

    let existing = col.find_one(doc! { "uuid": &id }).await.ok().flatten();
    let existing = match existing {
        Some(o) => o,
        None => return Ok(Response::not_found("Packaging option not found")),
    };

    if body.name.as_ref().map(|n| n.trim().is_empty()).unwrap_or(false) {
        return Ok(Response::bad_request("Name is required"));
    }
    if body.price.map(|p| p < 0).unwrap_or(false) {
        return Ok(Response::bad_request("Price can't be negative"));
    }

    let mut set_doc = Document::new();
    if let Some(v) = &body.name { set_doc.insert("name", v.trim()); }
    if let Some(v) = &body.description { set_doc.insert("description", v.trim()); }
    if let Some(v) = body.price { set_doc.insert("price", v); }
    if let Some(v) = body.enabled { set_doc.insert("enabled", v); }
    if let Some(v) = body.position { set_doc.insert("position", v as i32); }
    if let Some(v) = &body.image { set_doc.insert("image", v.trim()); }
    if let Some(v) = body.multi_item_only { set_doc.insert("multi_item_only", v); }

    if set_doc.is_empty() {
        return Ok(HttpResponse::Ok().content_type("application/json").json(existing));
    }

    if let Err(error) = col.update_one(doc! { "uuid": &id }, doc! { "$set": set_doc }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "packaging_option.updated", "packaging_option", &id, None,
    ).await;

    let updated = col.find_one(doc! { "uuid": &id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
