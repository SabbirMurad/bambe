use chrono::Utc;
use uuid::Uuid;
use mongodb::bson::doc;
use serde_json::json;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Review::{ProductReview, ReviewVote};

// Toggles the caller's "helpful" vote on a review. A second click withdraws
// it rather than piling up votes, and keeps helpful_count in sync with the
// vote rows it's denormalised from.
pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let review_id = path.into_inner();
    let db = MongoDB.connect();

    let vote_filter = doc! { "review_id": &review_id, "user_id": &user.user_id };
    let existing = db
        .collection::<ReviewVote>("review_vote")
        .find_one(vote_filter.clone())
        .await
        .ok()
        .flatten();

    let now_helpful = existing.is_none();

    if existing.is_some() {
        if let Err(error) = db.collection::<ReviewVote>("review_vote").delete_one(vote_filter).await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    } else {
        let vote = ReviewVote {
            uuid: Uuid::now_v7().to_string(),
            review_id: review_id.clone(),
            user_id: user.user_id.clone(),
            helpful: true,
            created_at: Utc::now().timestamp_millis(),
        };
        if let Err(error) = db.collection::<ReviewVote>("review_vote").insert_one(&vote).await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    let delta = if now_helpful { 1 } else { -1 };
    let result = db
        .collection::<ProductReview>("product_review")
        .update_one(doc! { "uuid": &review_id }, doc! { "$inc": { "helpful_count": delta } })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "helpful": now_helpful })))
}
