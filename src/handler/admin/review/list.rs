use std::collections::HashMap;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::{AccountCore, AccountRole};
use crate::model::Order::Order;
use crate::model::Product::Product;
use crate::model::Review::ProductReview;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | published | hidden
    status: Option<String>,
    // 1..=5, exact star rating
    rating: Option<u8>,
    // matched against product titles
    q: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

/*
  Reviews for the console, newest first, one entry per review as the
  customer wrote it: the rows of a whole-order review are folded into one
  entry that lists every product it sits on.
*/
pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<ProductReview>("product_review");

    let mut filter = doc! { "deleted_at": null };
    match query.status.as_deref() {
        Some("published") => { filter.insert("status", "Published"); }
        Some("hidden") => { filter.insert("status", "Rejected"); }
        _ => {}
    }
    if let Some(rating) = query.rating.filter(|r| (1..=5).contains(r)) {
        filter.insert("rating", rating as i32);
    }
    if let Some(q) = query.q.as_ref().map(|q| q.trim()).filter(|q| !q.is_empty()) {
        let mut ids = Vec::new();
        if let Ok(mut cursor) = db
            .collection::<Product>("product")
            .find(doc! { "title": { "$regex": regex_escape(q), "$options": "i" } })
            .await
        {
            while let Some(Ok(p)) = cursor.next().await {
                ids.push(p.uuid);
            }
        }
        filter.insert("product_id", doc! { "$in": ids });
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    // Fold each whole-order review's rows into one, then page over those.
    let pipeline = vec![
        doc! { "$match": filter },
        doc! { "$group": {
            "_id": { "$ifNull": ["$group_id", "$uuid"] },
            "uuid": { "$first": "$uuid" },
            "created_at": { "$first": "$created_at" },
        } },
        doc! { "$sort": { "created_at": -1, "_id": 1 } },
        doc! { "$facet": {
            "items": [ { "$skip": limit * (page - 1) }, { "$limit": limit } ],
            "total": [ { "$count": "n" } ],
        } },
    ];

    let mut keys: Vec<String> = Vec::new();
    let mut total: i64 = 0;
    match db.collection::<Document>("product_review").aggregate(pipeline).await {
        Ok(mut cursor) => {
            if let Some(Ok(facet)) = cursor.next().await {
                if let Ok(items) = facet.get_array("items") {
                    keys = items.iter().filter_map(|d| d.as_document()?.get_str("_id").ok().map(String::from)).collect();
                }
                if let Ok(t) = facet.get_array("total") {
                    total = t.first().and_then(|d| d.as_document()).and_then(|d| d.get_i32("n").ok()).unwrap_or(0) as i64;
                }
            }
        }
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    // Every row behind those keys — all of a group's products, not only the
    // ones a product search matched.
    let mut rows: Vec<ProductReview> = Vec::new();
    if !keys.is_empty() {
        if let Ok(mut cursor) = col
            .find(doc! { "$or": [ { "group_id": { "$in": &keys } }, { "uuid": { "$in": &keys } } ] })
            .await
        {
            while let Some(result) = cursor.next().await {
                match result {
                    Ok(r) => rows.push(r),
                    Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
                }
            }
        }
    }

    let product_ids: Vec<String> = rows.iter().map(|r| r.product_id.clone()).collect();
    let user_ids: Vec<String> = rows.iter().map(|r| r.user_id.clone()).collect();
    let order_ids: Vec<String> = rows.iter().filter_map(|r| r.order_id.clone()).collect();

    let mut products: HashMap<String, Product> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Product>("product").find(doc! { "uuid": { "$in": &product_ids } }).await {
        while let Some(Ok(p)) = cursor.next().await {
            products.insert(p.uuid.clone(), p);
        }
    }
    let mut authors: HashMap<String, AccountCore> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<AccountCore>("account_core").find(doc! { "uuid": { "$in": &user_ids } }).await {
        while let Some(Ok(a)) = cursor.next().await {
            authors.insert(a.uuid.clone(), a);
        }
    }
    let mut orders: HashMap<String, String> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Order>("order").find(doc! { "uuid": { "$in": &order_ids } }).await {
        while let Some(Ok(o)) = cursor.next().await {
            orders.insert(o.uuid, o.number);
        }
    }

    let items: Vec<_> = keys
        .iter()
        .filter_map(|key| {
            let group: Vec<&ProductReview> = rows
                .iter()
                .filter(|r| r.group_id.as_deref() == Some(key.as_str()) || (r.group_id.is_none() && &r.uuid == key))
                .collect();
            let first = group.first()?;
            let author = authors.get(&first.user_id);
            Some(json!({
                "uuid": first.uuid,
                "group_id": first.group_id,
                "scope": first.scope,
                "products": group.iter().map(|r| {
                    let p = products.get(&r.product_id);
                    json!({
                        "uuid": r.product_id,
                        "title": p.map(|p| p.title.clone()).unwrap_or_else(|| "Removed product".to_string()),
                        "slug": p.map(|p| p.slug.clone()),
                        "variant_label": r.variant_label,
                        "helpful_count": r.helpful_count,
                    })
                }).collect::<Vec<_>>(),
                "author": author.map(|a| a.username.clone()).unwrap_or_else(|| "Customer".to_string()),
                "author_email": author.map(|a| a.email_address.clone()),
                "user_id": first.user_id,
                "order_id": first.order_id,
                "order_number": first.order_id.as_ref().and_then(|id| orders.get(id).cloned()),
                "rating": first.rating,
                "title": first.title,
                "body": first.body,
                "status": first.status.to_string(),
                "rejection_reason": first.rejection_reason,
                "reply": first.reply,
                "created_at": first.created_at,
            }))
        })
        .collect();

    let published = col.count_documents(doc! { "deleted_at": null, "status": "Published" }).await.unwrap_or(0);
    let hidden = col.count_documents(doc! { "deleted_at": null, "status": "Rejected" }).await.unwrap_or(0);

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
        // Row counts, i.e. what product pages show, not reviews as written.
        "stats": { "published_rows": published, "hidden_rows": hidden },
    })))
}

fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()|[]{}^$".contains(c) { out.push('\\'); }
        out.push(c);
    }
    out
}
