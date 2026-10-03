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

/// The image documents for `ids`, keyed by uuid. Unreadable ones are skipped.
async fn load_images(db: &mongodb::Database, ids: &[String]) -> HashMap<String, ImageStruct> {
    let mut map = HashMap::new();
    if ids.is_empty() {
        return map;
    }

    let mut cursor = db
        .collection::<ImageStruct>("image")
        .find(doc! { "uuid": { "$in": ids } })
        .await
        .expect("image lookup");
    while let Some(result) = cursor.next().await {
        match result {
            Ok(i) => { map.insert(i.uuid.clone(), i); }
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
            }
        }
    }
    map
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
    while let Some(result) = variant_cursor.next().await {
        let v = match result {
            Ok(v) => v,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        variants.push(v);
    }
    // Oldest first, so the swatch row and the gallery below it stay in the
    // same order across reloads — the cursor has no order of its own.
    variants.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.uuid.cmp(&b.uuid)));

    let brand = match &product.brand_id {
        Some(id) => db.collection::<Brand>("brand").find_one(doc! { "uuid": id }).await.ok().flatten(),
        None => None,
    };

    /*
      The gallery is the variants' images, in variant order. Two variants of
      one colourway (a size run, say) share a photo, so the ids are deduped —
      otherwise a shirt in five sizes shows the same picture five times.
    */
    let mut image_ids: Vec<String> = Vec::new();
    for variant in &variants {
        if !image_ids.contains(&variant.image_id) {
            image_ids.push(variant.image_id.clone());
        }
    }

    let images = load_images(&db, &image_ids).await;

    let gallery: Vec<_> = image_ids
        .iter()
        .map(|id| json!({ "image_id": id, "image": images.get(id) }))
        .collect();

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
    let mut related_products = Vec::new();
    while let Some(result) = related_cursor.next().await {
        match result {
            Ok(p) => related_products.push(p),
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
            }
        }
    }

    // The whole image, not just its id, so the card can show the blur hash
    // while the photo loads — the same shape the listing endpoints send.
    let cover_ids: Vec<String> = related_products
        .iter()
        .filter_map(|p| p.cover_image_id.clone())
        .collect();
    let covers = load_images(&db, &cover_ids).await;

    let mut related = Vec::new();
    for p in related_products {
        let image = p.cover_image_id.as_ref().and_then(|id| covers.get(id));
        related.push(json!({
            "uuid": p.uuid,
            "slug": p.slug,
            "title": p.title,
            "image_id": p.cover_image_id,
            "image": image,
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
