use mongodb::bson::doc;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Notification;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;
    let db = MongoDB.connect();

    let result = db.collection::<Notification::AppNotification>("notification")
        .update_many(
            doc!{ "recipient_id": &user_id, "is_read": false },
            doc!{ "$set": { "is_read": true } },
        )
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("All notifications marked as read"))
}
