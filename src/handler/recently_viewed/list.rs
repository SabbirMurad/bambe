use std::collections::HashMap;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Cart::RecentlyViewed;
use crate::model::Product::Product;
use crate::model::ImageStruct;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    limit: Option<u32>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let limit = query.limit.unwrap_or(20).clamp(1, 50) as i64;

    let result = db
        .collection::<RecentlyViewed>("recently_viewed")
        .find(doc! { "user_id": &user.user_id })
        .sort(doc! { "viewed_at": -1 })
        .limit(limit)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut views = Vec::new();
    while let Some(Ok(v)) = cursor.next().await {
        views.push(v);
    }

    let product_ids: Vec<String> = views.iter().map(|v| v.product_id.clone()).collect();
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
        .filter_map(|p| p.images.iter().min_by_key(|i| i.position).map(|i| i.image_id.clone()))
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

    let items: Vec<_> = views
        .iter()
        .filter_map(|v| {
            let product = products.get(&v.product_id)?;
            let cover = product.images.iter().min_by_key(|i| i.position).and_then(|i| images.get(&i.image_id));
            Some(json!({
                "product_id": product.uuid,
                "slug": product.slug,
                "title": product.title,
                "image": cover,
                "price_min": product.price_min,
                "price_max": product.price_max,
                "compare_at_price": product.compare_at_price,
                "rating_average": product.rating_average,
                "viewed_at": v.viewed_at,
            }))
        })
        .collect();

    Ok(HttpResponse::Ok().content_type("application/json").json(items))
}
