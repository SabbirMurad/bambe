use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Review::{ProductReview, ReviewVote};

/*
  Deletes a review outright, with its helpful votes. Unlike hiding, this
  frees the products up: the customer can review that order again.
*/
pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let id = path.into_inner();

    let (filter, rows) = match super::rows_of(&db, &id).await {
        Some(found) => found,
        None => return Ok(Response::not_found("Review not found")),
    };

    let row_ids: Vec<String> = rows.iter().map(|r| r.uuid.clone()).collect();
    if let Err(error) = db.collection::<ProductReview>("product_review").delete_many(filter).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }
    if let Err(error) = db.collection::<ReviewVote>("review_vote").delete_many(doc! { "review_id": { "$in": &row_ids } }).await {
        log::error!("{:?}", error);
    }

    for row in &rows {
        crate::Handler::Review::refresh_rating(&db, &row.product_id).await;
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "review.deleted", "product_review", &id,
        rows.first().map(|r| format!("{}★ — {}", r.rating, r.body.chars().take(80).collect::<String>())),
    ).await;

    Ok(Response::ok_message("Review deleted"))
}
