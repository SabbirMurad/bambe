use chrono::Utc;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Address::Address;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let result = db
        .collection::<Address>("address")
        .update_one(
            doc! { "uuid": path.into_inner(), "user_id": &user.user_id, "deleted_at": null },
            doc! { "$set": { "deleted_at": Utc::now().timestamp_millis() } },
        )
        .await;

    match result {
        Ok(r) if r.matched_count == 0 => Ok(Response::not_found("Address not found")),
        Ok(_) => Ok(Response::ok_message("Address removed")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
