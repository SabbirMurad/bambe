use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Bson, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Category::Category;
use super::recompute_counts;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    parent_id: Option<String>,
    swatch: Option<String>,
    image_id: Option<String>,
    visible: Option<bool>,
    position: Option<u16>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Category>("category");
    let category_id = path.into_inner();

    if category_id == body.parent_id.clone().unwrap_or_default() {
        return Ok(Response::bad_request("A category can't be its own parent"));
    }

    if col.find_one(doc! { "uuid": &category_id }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Category not found"));
    }

    let mut set_doc = Document::new();
    if let Some(v) = &body.name { set_doc.insert("name", v); }
    if let Some(v) = &body.slug { set_doc.insert("slug", v); }
    if let Some(v) = &body.description { set_doc.insert("description", v); }
    if let Some(v) = &body.parent_id { set_doc.insert("parent_id", v); }
    if let Some(v) = &body.swatch { set_doc.insert("swatch", v); }
    // An empty image_id removes the picture (absent means "unchanged").
    if let Some(v) = &body.image_id {
        if v.trim().is_empty() { set_doc.insert("image_id", Bson::Null); }
        else { set_doc.insert("image_id", v); }
    }
    if let Some(v) = body.visible { set_doc.insert("visible", v); }
    if let Some(v) = body.position { set_doc.insert("position", v as i32); }
    set_doc.insert("modified_at", Utc::now().timestamp_millis());
    set_doc.insert("modified_by", admin.user_id);

    let result = col.update_one(doc! { "uuid": &category_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    if body.parent_id.is_some() {
        recompute_counts(&db).await;
    }

    let updated = col.find_one(doc! { "uuid": &category_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
