use mongodb::bson::doc;
use serde::Deserialize;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Notification;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    notification_id: String,
}

pub async fn task(
    req: HttpRequest,
    body: web::Json<ReqBody>,
) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;
    let db = MongoDB.connect();

    let result = db.collection::<Notification::AppNotification>("notification")
        .update_one(
            doc!{ "uuid": &body.notification_id, "recipient_id": &user_id },
            doc!{ "$set": { "is_read": true } },
        )
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("Marked as read"))
}
