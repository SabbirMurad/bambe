use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Product::{Product, ProductVariant};
use super::recompute_aggregates;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    sku: String,
    /// One value per product option, in the product's option order.
    #[serde(default)]
    options: Vec<String>,
    color_hex: Option<String>,
    image_id: String,
    #[serde(default)]
    extra_image_ids: Vec<String>,
    price: Cents,
    compare_at_price: Option<Cents>,
    #[serde(default)]
    stock: i32,
    #[serde(default = "default_low_stock")]
    low_stock_threshold: i32,
}
fn default_low_stock() -> i32 { 5 }

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let product_id = path.into_inner();

    let product = match db.collection::<Product>("product").find_one(doc! { "uuid": &product_id, "deleted_at": null }).await.ok().flatten() {
        Some(p) => p,
        None => return Ok(Response::not_found("Product not found")),
    };
    let pairs = match super::pair_values(&product.options, &body.options) {
        Ok(pairs) => pairs,
        Err(message) => return Ok(Response::bad_request(&message)),
    };

    if let Err(message) = super::check_compare_at(body.price, body.compare_at_price) {
        return Ok(Response::bad_request(message));
    }
    let extra_image_ids = match super::check_extra_photos(&body.image_id, &body.extra_image_ids) {
        Ok(ids) => ids,
        Err(message) => return Ok(Response::bad_request(&message)),
    };

    let now = Utc::now().timestamp_millis();
    let variant = ProductVariant {
        uuid: Uuid::now_v7().to_string(),
        product_id: product_id.clone(),
        sku: body.sku.clone(),
        options: pairs,
        color_hex: body.color_hex.clone(),
        image_id: body.image_id.clone(),
        extra_image_ids,
        price: body.price,
        compare_at_price: body.compare_at_price,
        stock: body.stock,
        low_stock_threshold: body.low_stock_threshold,
        created_at: now,
        modified_at: now,
    };

    if let Err(error) = db.collection::<ProductVariant>("product_variant").insert_one(&variant).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    recompute_aggregates(&db, &product_id).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(variant))
}
