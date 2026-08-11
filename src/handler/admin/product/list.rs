use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Product::Product;

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
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
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
    while let Some(Ok(p)) = cursor.next().await {
        products.push(p);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": products,
        "total": total,
        "page": page,
        "limit": limit,
    })))
}
