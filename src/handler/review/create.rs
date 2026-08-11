use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Review::{ProductReview, ReviewStatus};
use crate::model::Order::{Order, OrderItem};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    product_id: String,
    // Claimed order this review comes from. Verified against the caller's
    // own orders below — a client can't hand itself the "verified" badge by
    // just naming any order id.
    order_id: Option<String>,
    variant_id: Option<String>,
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

    let verified_order_id = match &body.order_id {
        Some(order_id) => {
            let owns_order = db
                .collection::<Order>("order")
                .find_one(doc! { "uuid": order_id, "user_id": &user.user_id })
                .await
                .ok()
                .flatten()
                .is_some();

            let bought_product = owns_order
                && db
                    .collection::<OrderItem>("order_item")
                    .find_one(doc! { "order_id": order_id, "product_id": &body.product_id })
                    .await
                    .ok()
                    .flatten()
                    .is_some();

            if bought_product { Some(order_id.clone()) } else { None }
        }
        None => None,
    };

    let now = Utc::now().timestamp_millis();
    let review = ProductReview {
        uuid: Uuid::now_v7().to_string(),
        product_id: body.product_id.clone(),
        user_id: user.user_id,
        order_id: verified_order_id,
        variant_id: body.variant_id.clone(),
        rating: body.rating,
        title: body.title.clone(),
        body: body.body.clone(),
        image_ids: body.image_ids.clone(),
        status: ReviewStatus::Pending,
        rejection_reason: None,
        helpful_count: 0,
        reply: None,
        created_at: now,
        modified_at: now,
        deleted_at: None,
    };

    let result = db.collection::<ProductReview>("product_review").insert_one(&review).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("Review submitted for moderation"))
}
