use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Coupon::{Coupon, DiscountType};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    description: Option<String>,
    discount_type: Option<DiscountType>,
    value: Option<i64>,
    max_discount: Option<Cents>,
    min_spend: Option<Cents>,
    total_limit: Option<u32>,
    per_user_limit: Option<u32>,
    starts_at: Option<i64>,
    ends_at: Option<i64>,
    enabled: Option<bool>,
    product_ids: Option<Vec<String>>,
    category_ids: Option<Vec<String>>,
    first_order_only: Option<bool>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Coupon>("coupon");
    let coupon_id = path.into_inner();

    if col.find_one(doc! { "uuid": &coupon_id }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Coupon not found"));
    }

    let mut set_doc = Document::new();
    if let Some(v) = &body.description { set_doc.insert("description", v); }
    if let Some(v) = &body.discount_type { set_doc.insert("discount_type", mongodb::bson::to_bson(v).unwrap()); }
    if let Some(v) = body.value { set_doc.insert("value", v); }
    if let Some(v) = body.max_discount { set_doc.insert("max_discount", v); }
    if let Some(v) = body.min_spend { set_doc.insert("min_spend", v); }
    if let Some(v) = body.total_limit { set_doc.insert("total_limit", v); }
    if let Some(v) = body.per_user_limit { set_doc.insert("per_user_limit", v); }
    if let Some(v) = body.starts_at { set_doc.insert("starts_at", v); }
    if body.ends_at.is_some() { set_doc.insert("ends_at", body.ends_at); }
    if let Some(v) = body.enabled { set_doc.insert("enabled", v); }
    if let Some(v) = &body.product_ids { set_doc.insert("product_ids", v); }
    if let Some(v) = &body.category_ids { set_doc.insert("category_ids", v); }
    if let Some(v) = body.first_order_only { set_doc.insert("first_order_only", v); }
    set_doc.insert("modified_at", Utc::now().timestamp_millis());

    let result = col.update_one(doc! { "uuid": &coupon_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let updated = col.find_one(doc! { "uuid": &coupon_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
