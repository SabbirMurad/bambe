use chrono::Utc;
use uuid::Uuid;
use mongodb::bson::{doc, Document};
use serde::Deserialize;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    fcm_token: String,
}

pub async fn task(
    req: HttpRequest,
    body: web::Json<ReqBody>,
) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;

    if body.fcm_token.trim().is_empty() {
        return Ok(Response::bad_request("fcm_token is required"));
    }

    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();

    // Upsert on (user_id, token) so the same device only ever has one record,
    // while a user can register multiple devices.
    let result = db.collection::<Document>("fcm_token")
        .update_one(
            doc!{ "user_id": &user_id, "token": &body.fcm_token },
            doc!{
                "$set": { "updated_at": now },
                "$setOnInsert": {
                    "uuid": Uuid::now_v7().to_string(),
                    "user_id": &user_id,
                    "token": &body.fcm_token,
                    "created_at": now,
                }
            },
        )
        .upsert(true)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("FCM token registered"))
}
