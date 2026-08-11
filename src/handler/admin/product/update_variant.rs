use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Product::ProductVariant;
use super::recompute_aggregates;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    sku: Option<String>,
    size: Option<String>,
    color: Option<String>,
    color_hex: Option<String>,
    image_id: Option<String>,
    price: Option<Cents>,
    compare_at_price: Option<Cents>,
    stock: Option<i32>,
    low_stock_threshold: Option<i32>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<ProductVariant>("product_variant");
    let variant_id = path.into_inner();

    let existing = col.find_one(doc! { "uuid": &variant_id }).await.ok().flatten();
    let existing = match existing {
        Some(v) => v,
        None => return Ok(Response::not_found("Variant not found")),
    };

    let mut set_doc = Document::new();
    if let Some(v) = &body.sku { set_doc.insert("sku", v); }
    if let Some(v) = &body.size { set_doc.insert("size", v); }
    if let Some(v) = &body.color { set_doc.insert("color", v); }
    if let Some(v) = &body.color_hex { set_doc.insert("color_hex", v); }
    if let Some(v) = &body.image_id { set_doc.insert("image_id", v); }
    if let Some(v) = body.price { set_doc.insert("price", v); }
    if let Some(v) = body.compare_at_price { set_doc.insert("compare_at_price", v); }
    if let Some(v) = body.stock { set_doc.insert("stock", v); }
    if let Some(v) = body.low_stock_threshold { set_doc.insert("low_stock_threshold", v); }
    set_doc.insert("modified_at", Utc::now().timestamp_millis());

    let result = col.update_one(doc! { "uuid": &variant_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    recompute_aggregates(&db, &existing.product_id).await;

    let updated = col.find_one(doc! { "uuid": &variant_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
