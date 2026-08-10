use std::collections::HashMap;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::model::Product::{Product, ProductVariant, Brand};
use crate::model::Category::Category;
use crate::model::ImageStruct;

/// Walks `parent_id` up from `category_id` to build the breadcrumb, root
/// first. Capped at 10 hops — deeper than any real category tree, just a
/// backstop against a cyclic parent_id corrupting the page into a spin.
async fn breadcrumb(db: &mongodb::Database, category_id: &str) -> Vec<serde_json::Value> {
    let col = db.collection::<Category>("category");
    let mut chain = Vec::new();
    let mut current = col.find_one(doc! { "uuid": category_id }).await.ok().flatten();
    let mut hops = 0;

    while let Some(c) = current {
        chain.push(json!({ "uuid": c.uuid, "name": c.name, "slug": c.slug }));
        hops += 1;
        if hops >= 10 {
            break;
        }
        current = match &c.parent_id {
            Some(parent_id) => col.find_one(doc! { "uuid": parent_id }).await.ok().flatten(),
            None => None,
        };
    }

    chain.reverse();
    chain
}

pub async fn task(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let slug = path.into_inner();
    let db = MongoDB.connect();

    let product = db
        .collection::<Product>("product")
        .find_one(doc! { "slug": &slug, "status": "Active", "deleted_at": null })
        .await;

    let product = match product {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(Response::not_found("Product not found")),
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    };

    let mut variant_cursor = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "product_id": &product.uuid })
        .await
        .expect("variant lookup");
    let mut variants = Vec::new();
    while let Some(Ok(v)) = variant_cursor.next().await {
        variants.push(v);
    }

    let brand = match &product.brand_id {
        Some(id) => db.collection::<Brand>("brand").find_one(doc! { "uuid": id }).await.ok().flatten(),
        None => None,
    };

    let image_ids: Vec<String> = product.images.iter().map(|i| i.image_id.clone()).collect();
    let images: HashMap<String, ImageStruct> = if image_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<ImageStruct>("image")
            .find(doc! { "uuid": { "$in": &image_ids } })
            .await
            .expect("image lookup");
        let mut map = HashMap::new();
        while let Some(Ok(i)) = cursor.next().await {
            map.insert(i.uuid.clone(), i);
        }
        map
    };

    let mut gallery: Vec<_> = product
        .images
        .iter()
        .map(|pi| {
            json!({
                "image": images.get(&pi.image_id),
                "alt": pi.alt,
                "position": pi.position,
            })
        })
        .collect();
    gallery.sort_by_key(|v| v["position"].as_i64().unwrap_or(0));

    let breadcrumb = breadcrumb(&db, &product.category_id).await;

    let mut related_cursor = db
        .collection::<Product>("product")
        .find(doc! {
            "category_id": &product.category_id,
            "uuid": { "$ne": &product.uuid },
            "status": "Active",
            "deleted_at": null,
        })
        .sort(doc! { "sold_count": -1 })
        .limit(8)
        .await
        .expect("related lookup");
    let mut related = Vec::new();
    while let Some(Ok(p)) = related_cursor.next().await {
        let cover_id = p.images.iter().min_by_key(|i| i.position).map(|i| i.image_id.clone());
        related.push(json!({
            "uuid": p.uuid,
            "slug": p.slug,
            "title": p.title,
            "image_id": cover_id,
            "price_min": p.price_min,
            "price_max": p.price_max,
            "compare_at_price": p.compare_at_price,
            "rating_average": p.rating_average,
            "rating_count": p.rating_count,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "uuid": product.uuid,
        "slug": product.slug,
        "title": product.title,
        "description": product.description,
        "brand": brand.map(|b| json!({ "uuid": b.uuid, "name": b.name, "slug": b.slug, "logo_image_id": b.logo_image_id })),
        "breadcrumb": breadcrumb,
        "tags": product.tags,
        "gallery": gallery,
        "variants": variants,
        "price_min": product.price_min,
        "price_max": product.price_max,
        "compare_at_price": product.compare_at_price,
        "in_stock": product.in_stock,
        "rating_average": product.rating_average,
        "rating_count": product.rating_count,
        "sold_count": product.sold_count,
        "related": related,
    })))
}
