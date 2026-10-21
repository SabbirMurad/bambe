use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Review::ProductReview;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    order_id: String,
}

/*
  Which products on one of the caller's orders they've already reviewed,
  so the orders page can mark them and stop offering them again.
*/
pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let mut product_ids = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<ProductReview>("product_review")
        .find(doc! { "user_id": &user.user_id, "order_id": &query.order_id, "deleted_at": null })
        .await
    {
        while let Some(Ok(r)) = cursor.next().await {
            product_ids.push(r.product_id);
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!(product_ids)))
}
