use std::collections::HashSet;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Review::ProductReview;
use crate::model::Order::{Order, OrderItem};

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    product_id: String,
}

/*
  The caller's delivered orders that contain this product and haven't had
  it reviewed yet, newest first. The product page offers "Write a review"
  only when this is non-empty — reviews are for people who bought it.
*/
pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let mut orders = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<Order>("order")
        .find(doc! { "user_id": &user.user_id, "fulfilment_status": "Delivered" })
        .sort(doc! { "placed_at": -1 })
        .await
    {
        while let Some(Ok(o)) = cursor.next().await {
            orders.push(o);
        }
    }
    let order_ids: Vec<String> = orders.iter().map(|o| o.uuid.clone()).collect();
    if order_ids.is_empty() {
        return Ok(HttpResponse::Ok().content_type("application/json").json(json!([])));
    }

    let mut bought_in = HashSet::new();
    if let Ok(mut cursor) = db
        .collection::<OrderItem>("order_item")
        .find(doc! { "order_id": { "$in": &order_ids }, "product_id": &query.product_id })
        .await
    {
        while let Some(Ok(line)) = cursor.next().await {
            bought_in.insert(line.order_id);
        }
    }

    let mut reviewed_in = HashSet::new();
    if let Ok(mut cursor) = db
        .collection::<ProductReview>("product_review")
        .find(doc! { "user_id": &user.user_id, "product_id": &query.product_id, "deleted_at": null })
        .await
    {
        while let Some(Ok(r)) = cursor.next().await {
            if let Some(id) = r.order_id { reviewed_in.insert(id); }
        }
    }

    let eligible: Vec<_> = orders
        .iter()
        .filter(|o| bought_in.contains(&o.uuid) && !reviewed_in.contains(&o.uuid))
        .map(|o| json!({ "order_id": o.uuid, "number": o.number }))
        .collect();

    Ok(HttpResponse::Ok().content_type("application/json").json(eligible))
}
