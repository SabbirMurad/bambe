use std::collections::HashSet;
use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Review::{ProductReview, ReviewScope, ReviewStatus};
use crate::model::Order::{FulfilmentStatus, Order, OrderItem};

/*
  Only a customer who bought the thing can review it, so every review
  names the delivered order it comes from.

  `product_id` set: a review of that one product.
  `product_id` absent: a review of the whole order — written as one row
  per product still unreviewed on it, sharing a `group_id`, so it shows
  on (and counts towards) each of those products.
*/
#[derive(Debug, Deserialize)]
pub struct ReqBody {
    order_id: String,
    product_id: Option<String>,
    rating: u8,
    title: Option<String>,
    body: String,
    #[serde(default)]
    image_ids: Vec<String>,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    if !(1..=5).contains(&body.rating) {
        return Ok(Response::bad_request("Rating must be between 1 and 5"));
    }

    if body.body.trim().is_empty() {
        return Ok(Response::bad_request("Review text is required"));
    }

    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &body.order_id, "user_id": &user.user_id })
        .await
        .ok()
        .flatten();
    let order = match order {
        Some(o) => o,
        None => return Ok(Response::not_found("Order not found")),
    };
    if order.fulfilment_status != FulfilmentStatus::Delivered {
        return Ok(Response::bad_request("You can review an order once it has been delivered"));
    }

    let mut lines = Vec::new();
    if let Ok(mut cursor) = db.collection::<OrderItem>("order_item").find(doc! { "order_id": &order.uuid }).await {
        while let Some(result) = cursor.next().await {
            match result {
                Ok(line) => lines.push(line),
                Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
            }
        }
    }

    // Products on this order the caller has already reviewed, either alone
    // or as part of an earlier whole-order review.
    let mut reviewed = HashSet::new();
    if let Ok(mut cursor) = db
        .collection::<ProductReview>("product_review")
        .find(doc! { "user_id": &user.user_id, "order_id": &order.uuid, "deleted_at": null })
        .await
    {
        while let Some(Ok(r)) = cursor.next().await {
            reviewed.insert(r.product_id);
        }
    }

    // One line per product: the same product bought in two shades is one
    // product to review, and the first line's variant speaks for it.
    let mut seen = HashSet::new();
    let targets: Vec<&OrderItem> = lines
        .iter()
        .filter(|line| match &body.product_id {
            Some(id) => &line.product_id == id,
            None => true,
        })
        .filter(|line| seen.insert(line.product_id.clone()))
        .collect();

    if targets.is_empty() {
        return Ok(Response::bad_request("That product isn't on this order"));
    }
    let targets: Vec<&OrderItem> = targets.into_iter().filter(|line| !reviewed.contains(&line.product_id)).collect();
    if targets.is_empty() {
        return Ok(Response::bad_request(if body.product_id.is_some() {
            "You've already reviewed this product from this order"
        } else {
            "You've already reviewed everything on this order"
        }));
    }

    let scope = if body.product_id.is_some() { ReviewScope::Product } else { ReviewScope::Order };
    let group_id = match scope {
        ReviewScope::Order => Some(Uuid::now_v7().to_string()),
        ReviewScope::Product => None,
    };
    let title = body.title.as_ref().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let now = Utc::now().timestamp_millis();

    let reviews: Vec<ProductReview> = targets
        .iter()
        .map(|line| ProductReview {
            uuid: Uuid::now_v7().to_string(),
            product_id: line.product_id.clone(),
            user_id: user.user_id.clone(),
            order_id: Some(order.uuid.clone()),
            variant_id: Some(line.variant_id.clone()),
            variant_label: line.variant_label.clone(),
            scope: scope.clone(),
            group_id: group_id.clone(),
            rating: body.rating,
            title: title.clone(),
            body: body.body.trim().to_string(),
            image_ids: body.image_ids.clone(),
            status: ReviewStatus::Published,
            rejection_reason: None,
            helpful_count: 0,
            reply: None,
            created_at: now,
            modified_at: now,
            deleted_at: None,
        })
        .collect();

    if let Err(error) = db.collection::<ProductReview>("product_review").insert_many(&reviews).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    for review in &reviews {
        super::refresh_rating(&db, &review.product_id).await;
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "message": "Review published",
        "product_ids": reviews.iter().map(|r| r.product_id.clone()).collect::<Vec<_>>(),
    })))
}
