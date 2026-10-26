use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::require_shopper;
use crate::model::Cart::WishlistItem;
use crate::model::Product::{Product, ProductVariant};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    product_id: String,
    variant_id: Option<String>,
    #[serde(default = "default_true")]
    notify_on_restock: bool,
    #[serde(default = "default_true")]
    notify_on_price_drop: bool,
}
fn default_true() -> bool { true }

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_shopper(&req)?;
    let db = MongoDB.connect();

    let col = db.collection::<WishlistItem>("wishlist_item");
    let mut existing_filter = doc! { "user_id": &user.user_id, "product_id": &body.product_id };
    match &body.variant_id {
        Some(id) => { existing_filter.insert("variant_id", id); }
        None => { existing_filter.insert("variant_id", mongodb::bson::Bson::Null); }
    }

    if let Some(existing) = col.find_one(existing_filter).await.ok().flatten() {
        return Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "uuid": existing.uuid })));
    }

    let price = match &body.variant_id {
        Some(variant_id) => db
            .collection::<ProductVariant>("product_variant")
            .find_one(doc! { "uuid": variant_id })
            .await
            .ok()
            .flatten()
            .map(|v| v.price),
        None => db
            .collection::<Product>("product")
            .find_one(doc! { "uuid": &body.product_id })
            .await
            .ok()
            .flatten()
            .map(|p| p.price_min),
    };

    let price = match price {
        Some(p) => p,
        None => return Ok(Response::not_found("Product not found")),
    };

    let item = WishlistItem {
        uuid: Uuid::now_v7().to_string(),
        user_id: user.user_id,
        product_id: body.product_id.clone(),
        variant_id: body.variant_id.clone(),
        price_when_added: price,
        notify_on_restock: body.notify_on_restock,
        notify_on_price_drop: body.notify_on_price_drop,
        created_at: Utc::now().timestamp_millis(),
    };

    let result = col.insert_one(&item).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "uuid": item.uuid })))
}
