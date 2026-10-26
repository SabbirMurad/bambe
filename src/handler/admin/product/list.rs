use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Product::{Product, ProductVariant};

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | active | draft | archived | out_of_stock
    tab: Option<String>,
    category: Option<String>,
    q: Option<String>,
    sort: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let col = db.collection::<Product>("product");

    let mut filter = doc! { "deleted_at": null };
    match query.tab.as_deref().unwrap_or("all") {
        "active" => { filter.insert("status", "Active"); }
        "draft" => { filter.insert("status", "Draft"); }
        "archived" => { filter.insert("status", "Archived"); }
        "out_of_stock" => { filter.insert("in_stock", false); }
        _ => {}
    }
    if let Some(category) = &query.category {
        filter.insert("category_id", category);
    }
    if let Some(q) = &query.q {
        filter.insert("$or", vec![
            doc! { "title": { "$regex": q, "$options": "i" } },
            doc! { "slug": { "$regex": q, "$options": "i" } },
        ]);
    }

    let sort = match query.sort.as_deref() {
        Some("price_asc") => doc! { "price_min": 1 },
        Some("price_desc") => doc! { "price_min": -1 },
        Some("stock_asc") => doc! { "in_stock": 1 },
        Some("oldest") => doc! { "created_at": 1 },
        _ => doc! { "created_at": -1 },
    };

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);
    let result = col.find(filter).sort(sort).skip((limit * (page - 1)) as u64).limit(limit).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut products = Vec::new();
    while let Some(next) = cursor.next().await {
        match next {
            Ok(p) => products.push(p),
            Err(error) => log::error!("admin product list: undeserialisable product skipped: {:?}", error),
        }
    }

    /*
      The console's Stock column wants a number, and `Product` only carries
      the `in_stock` flag — the count lives on the variants. One `$in` over
      product_variant for the page, rather than a query per row or a column
      that says nothing.
    */
    let product_ids: Vec<String> = products.iter().map(|p| p.uuid.clone()).collect();
    let mut stock_by_product: std::collections::HashMap<String, (i32, i32, u32)> =
        std::collections::HashMap::new();
    if !product_ids.is_empty() {
        let mut variant_cursor = db
            .collection::<ProductVariant>("product_variant")
            .find(doc! { "product_id": { "$in": &product_ids } })
            .await
            .expect("variant lookup");
        while let Some(result) = variant_cursor.next().await {
            let v = match result {
                Ok(v) => v,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            let entry = stock_by_product.entry(v.product_id.clone()).or_insert((0, 0, 0));
            entry.0 += v.stock;
            // The lowest headroom over any one variant is what "low stock"
            // means for the product as a whole: one colourway about to run
            // out is a restock, even when the others are full.
            let headroom = v.stock - v.low_stock_threshold;
            if entry.2 == 0 || headroom < entry.1 { entry.1 = headroom; }
            entry.2 += 1;
        }
    }

    let items: Vec<serde_json::Value> = products
        .iter()
        .map(|product| {
            let mut view = serde_json::to_value(product).unwrap_or_else(|_| json!({}));
            let (stock, headroom, variants) = stock_by_product
                .get(&product.uuid)
                .copied()
                .unwrap_or((0, 0, 0));
            view["stock_total"] = json!(stock);
            view["variant_count"] = json!(variants);
            view["low_stock"] = json!(variants > 0 && stock > 0 && headroom <= 0);
            view
        })
        .collect();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
    })))
}
