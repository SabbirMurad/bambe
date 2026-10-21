use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Review::ProductReview;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    // "Published" puts it back on the product pages; "Rejected" hides it.
    status: String,
    // Kept for staff only — the customer is not told.
    reason: Option<String>,
}

/*
  Hides a review from the storefront, or puts a hidden one back. A hidden
  review still counts as "reviewed" for that order, so the customer can't
  simply post it again; deleting is what clears that.
*/
pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let id = path.into_inner();

    let (status, reason) = match body.status.as_str() {
        "Published" => ("Published", None),
        "Rejected" => ("Rejected", body.reason.as_ref().map(|r| r.trim().to_string()).filter(|r| !r.is_empty())),
        _ => return Ok(Response::bad_request("Status must be Published or Rejected")),
    };

    let (filter, rows) = match super::rows_of(&db, &id).await {
        Some(found) => found,
        None => return Ok(Response::not_found("Review not found")),
    };

    let result = db
        .collection::<ProductReview>("product_review")
        .update_many(filter, doc! { "$set": {
            "status": status,
            "rejection_reason": reason.clone(),
            "modified_at": Utc::now().timestamp_millis(),
        } })
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    for row in &rows {
        crate::Handler::Review::refresh_rating(&db, &row.product_id).await;
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name,
        if status == "Published" { "review.published" } else { "review.hidden" },
        "product_review", &id, reason,
    ).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "status": status, "rows": rows.len() })))
}
