use std::collections::HashMap;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Cart::WishlistItem;
use crate::model::Product::Product;
use crate::model::ImageStruct;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let result = db
        .collection::<WishlistItem>("wishlist_item")
        .find(doc! { "user_id": &user.user_id })
        .sort(doc! { "created_at": -1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut wishlist_items = Vec::new();
    while let Some(Ok(w)) = cursor.next().await {
        wishlist_items.push(w);
    }

    let product_ids: Vec<String> = wishlist_items.iter().map(|w| w.product_id.clone()).collect();
    let products: HashMap<String, Product> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<Product>("product")
            .find(doc! { "uuid": { "$in": &product_ids } })
            .await
            .expect("product lookup");
        let mut map = HashMap::new();
        while let Some(Ok(p)) = cursor.next().await {
            map.insert(p.uuid.clone(), p);
        }
        map
    };

    let cover_image_ids: Vec<String> = products
        .values()
        .filter_map(|p| p.cover_image_id.clone())
        .collect();
    let images: HashMap<String, ImageStruct> = if cover_image_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<ImageStruct>("image")
            .find(doc! { "uuid": { "$in": &cover_image_ids } })
            .await
            .expect("image lookup");
        let mut map = HashMap::new();
        while let Some(Ok(i)) = cursor.next().await {
            map.insert(i.uuid.clone(), i);
        }
        map
    };

    let items: Vec<_> = wishlist_items
        .iter()
        .map(|w| {
            let product = products.get(&w.product_id);
            let cover = product.and_then(|p| p.cover_image_id.as_ref()).and_then(|id| images.get(id));

            json!({
                "uuid": w.uuid,
                "product_id": w.product_id,
                "variant_id": w.variant_id,
                "title": product.map(|p| p.title.clone()),
                "slug": product.map(|p| p.slug.clone()),
                "image": cover,
                "price_min": product.map(|p| p.price_min),
                "price_max": product.map(|p| p.price_max),
                "compare_at_price": product.and_then(|p| p.compare_at_price),
                "price_when_added": w.price_when_added,
                "price_dropped": product.map(|p| p.price_min < w.price_when_added).unwrap_or(false),
                "in_stock": product.map(|p| p.in_stock).unwrap_or(false),
                "notify_on_restock": w.notify_on_restock,
                "notify_on_price_drop": w.notify_on_price_drop,
                "created_at": w.created_at,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().content_type("application/json").json(items))
}
