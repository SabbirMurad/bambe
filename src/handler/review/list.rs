use std::collections::HashMap;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Review::ProductReview;
use crate::model::Account::AccountCore;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    pub product_id: String,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    // helpful | newest (default)
    pub sort: Option<String>,
}

pub async fn task(query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();
    let filter = doc! { "product_id": &query.product_id, "status": "Published" };

    let sort = match query.sort.as_deref() {
        Some("helpful") => doc! { "helpful_count": -1 },
        _ => doc! { "created_at": -1 },
    };

    let limit = query.limit.unwrap_or(10).clamp(1, 50) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;
    let skip = limit * (page - 1);

    let total = db
        .collection::<ProductReview>("product_review")
        .count_documents(filter.clone())
        .await
        .unwrap_or(0);

    let result = db
        .collection::<ProductReview>("product_review")
        .find(filter.clone())
        .sort(sort)
        .skip(skip as u64)
        .limit(limit)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut reviews = Vec::new();
    while let Some(result) = cursor.next().await {
        let r = match result {
            Ok(r) => r,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        reviews.push(r);
    }

    let user_ids: Vec<String> = reviews.iter().map(|r| r.user_id.clone()).collect();
    let authors: HashMap<String, String> = if user_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<AccountCore>("account_core")
            .find(doc! { "uuid": { "$in": &user_ids } })
            .await
            .expect("author lookup");
        let mut map = HashMap::new();
        while let Some(result) = cursor.next().await {
            let a = match result {
                Ok(a) => a,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            map.insert(a.uuid.clone(), a.username);
        }
        map
    };

    let items: Vec<_> = reviews
        .iter()
        .map(|r| {
            json!({
                "uuid": r.uuid,
                "author": authors.get(&r.user_id).cloned().unwrap_or_else(|| "Customer".to_string()),
                "verified_purchase": r.order_id.is_some(),
                "variant_id": r.variant_id,
                "variant_label": r.variant_label,
                "scope": r.scope,
                "rating": r.rating,
                "title": r.title,
                "body": r.body,
                "image_ids": r.image_ids,
                "helpful_count": r.helpful_count,
                "reply": r.reply,
                "created_at": r.created_at,
            })
        })
        .collect();

    // One-shot breakdown of how many 1..5 star reviews this product has,
    // over the same (published, this product) filter as the list above.
    let pipeline = vec![
        doc! { "$match": filter },
        doc! { "$group": { "_id": "$rating", "count": { "$sum": 1 } } },
    ];
    let mut breakdown = [0u64; 5];
    if let Ok(mut cursor) = db.collection::<Document>("product_review").aggregate(pipeline).await {
        while let Some(result) = cursor.next().await {
            let d = match result {
                Ok(d) => d,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            if let (Some(star), Some(count)) = (d.get_i32("_id").ok(), d.get_i32("count").ok()) {
                if (1..=5).contains(&star) {
                    breakdown[(star - 1) as usize] = count as u64;
                }
            }
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
        "rating_breakdown": {
            "1": breakdown[0],
            "2": breakdown[1],
            "3": breakdown[2],
            "4": breakdown[3],
            "5": breakdown[4],
        },
    })))
}
