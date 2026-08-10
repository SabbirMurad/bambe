use std::collections::{HashMap, HashSet};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::model::Product::{Product, ProductVariant, Brand};
use crate::model::Category::Category;
use crate::model::ImageStruct;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // Category uuid or slug. Matches the category itself and every
    // descendant, so filtering on "Footwear" also picks up "Boots".
    pub category: Option<String>,
    pub brand: Option<String>,
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    // Minimum rating_average, e.g. 4 for the "4 stars & up" radio.
    pub rating: Option<f32>,
    pub size: Option<String>,
    pub color: Option<String>,
    pub tag: Option<String>,
    pub q: Option<String>,
    // relevance | price_asc | price_desc | newest | rating | best_selling
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Every category matching `key` (by uuid or slug) plus all of its
/// descendants, so a listing scoped to a parent still returns children's
/// products. The whole collection is small enough to pull in one shot and
/// walk in memory rather than round-tripping per level.
async fn resolve_category_ids(db: &mongodb::Database, key: &str) -> Vec<String> {
    let all: Vec<Category> = match db.collection::<Category>("category").find(doc! {}).await {
        Ok(mut cursor) => {
            let mut out = Vec::new();
            while let Some(Ok(c)) = cursor.next().await {
                out.push(c);
            }
            out
        }
        Err(_) => return vec![key.to_string()],
    };

    let root = all.iter().find(|c| c.uuid == key || c.slug == key);
    let root_id = match root {
        Some(c) => c.uuid.clone(),
        None => return vec![key.to_string()],
    };

    let mut ids = vec![root_id.clone()];
    let mut frontier = vec![root_id];
    loop {
        let children: Vec<String> = all
            .iter()
            .filter(|c| c.parent_id.as_deref().map(|p| frontier.contains(&p.to_string())).unwrap_or(false))
            .map(|c| c.uuid.clone())
            .collect();

        if children.is_empty() {
            break;
        }

        ids.extend(children.iter().cloned());
        frontier = children;
    }

    ids
}

pub async fn task(query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();
    let mut filter = doc! { "status": "Active", "deleted_at": null };

    if let Some(category) = &query.category {
        let ids = resolve_category_ids(&db, category).await;
        filter.insert("category_id", doc! { "$in": ids });
    }

    if let Some(brand) = &query.brand {
        filter.insert("brand_id", brand);
    }

    if query.price_min.is_some() || query.price_max.is_some() {
        // Overlap test: the product's [price_min, price_max] range touches
        // the requested one, not just its cheapest variant.
        if let Some(max) = query.price_max {
            filter.insert("price_min", doc! { "$lte": max });
        }
        if let Some(min) = query.price_min {
            filter.insert("price_max", doc! { "$gte": min });
        }
    }

    if let Some(rating) = query.rating {
        filter.insert("rating_average", doc! { "$gte": rating });
    }

    if let Some(tag) = &query.tag {
        filter.insert("tags", tag);
    }

    if let Some(q) = &query.q {
        filter.insert(
            "$or",
            vec![
                doc! { "title": { "$regex": q, "$options": "i" } },
                doc! { "description": { "$regex": q, "$options": "i" } },
                doc! { "tags": { "$regex": q, "$options": "i" } },
            ],
        );
    }

    if query.size.is_some() || query.color.is_some() {
        let mut variant_filter = Document::new();
        if let Some(size) = &query.size {
            variant_filter.insert("size", size);
        }
        if let Some(color) = &query.color {
            variant_filter.insert("color", color);
        }

        let product_ids: HashSet<String> = match db
            .collection::<ProductVariant>("product_variant")
            .find(variant_filter)
            .await
        {
            Ok(mut cursor) => {
                let mut ids = HashSet::new();
                while let Some(Ok(v)) = cursor.next().await {
                    ids.insert(v.product_id);
                }
                ids
            }
            Err(_) => HashSet::new(),
        };

        filter.insert("uuid", doc! { "$in": product_ids.into_iter().collect::<Vec<_>>() });
    }

    let sort = match query.sort.as_deref() {
        Some("price_asc") => doc! { "price_min": 1 },
        Some("price_desc") => doc! { "price_min": -1 },
        Some("rating") => doc! { "rating_average": -1 },
        Some("best_selling") => doc! { "sold_count": -1 },
        // "relevance" has no text index to rank against yet, so it falls
        // back to newest — still a reasonable default with a search term.
        _ => doc! { "created_at": -1 },
    };

    let limit = query.limit.unwrap_or(24).clamp(1, 60) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;
    let skip = limit * (page - 1);

    let total = db
        .collection::<Product>("product")
        .count_documents(filter.clone())
        .await
        .unwrap_or(0);

    let result = db
        .collection::<Product>("product")
        .find(filter)
        .sort(sort)
        .skip(skip as u64)
        .limit(limit)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut products = Vec::new();
    while let Some(Ok(p)) = cursor.next().await {
        products.push(p);
    }

    // Resolve brand names and cover images in bulk rather than per product.
    let brand_ids: Vec<String> = products.iter().filter_map(|p| p.brand_id.clone()).collect();
    let brands: HashMap<String, Brand> = if brand_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<Brand>("brand")
            .find(doc! { "uuid": { "$in": &brand_ids } })
            .await
            .expect("brand lookup");
        let mut map = HashMap::new();
        while let Some(Ok(b)) = cursor.next().await {
            map.insert(b.uuid.clone(), b);
        }
        map
    };

    let cover_image_ids: Vec<String> = products
        .iter()
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

    let items: Vec<_> = products
        .iter()
        .map(|p| {
            let cover = p
                .images
                .iter()
                .min_by_key(|i| i.position)
                .and_then(|i| images.get(&i.image_id));

            json!({
                "uuid": p.uuid,
                "slug": p.slug,
                "title": p.title,
                "brand": p.brand_id.as_ref().and_then(|id| brands.get(id)).map(|b| json!({ "uuid": b.uuid, "name": b.name, "slug": b.slug })),
                "category_id": p.category_id,
                "image": cover,
                "price_min": p.price_min,
                "price_max": p.price_max,
                "compare_at_price": p.compare_at_price,
                "in_stock": p.in_stock,
                "rating_average": p.rating_average,
                "rating_count": p.rating_count,
                "sold_count": p.sold_count,
                "created_at": p.created_at,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
    })))
}
