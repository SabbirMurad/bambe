use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::{doc, Bson};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Review::ProductReview;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    // Empty removes the reply.
    body: String,
}

/* The shop's one public reply, shown under the review on the product page. */
pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let id = path.into_inner();

    let (filter, _) = match super::rows_of(&db, &id).await {
        Some(found) => found,
        None => return Ok(Response::not_found("Review not found")),
    };

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    let now = Utc::now().timestamp_millis();
    let text = body.body.trim();
    let reply = if text.is_empty() {
        Bson::Null
    } else {
        Bson::Document(doc! { "body": text, "replied_by": &name, "replied_at": now })
    };

    let result = db
        .collection::<ProductReview>("product_review")
        .update_many(filter, doc! { "$set": { "reply": reply.clone(), "modified_at": now } })
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name,
        if text.is_empty() { "review.reply_removed" } else { "review.replied" },
        "product_review", &id, None,
    ).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({ "reply": reply })))
}
