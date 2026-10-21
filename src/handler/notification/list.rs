use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::{Notification, ImageStruct};

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;
    let db = MongoDB.connect();

    let result = db.collection::<Notification::AppNotification>("notification")
        .find(doc!{ "recipient_id": &user_id })
        .sort(doc!{ "created_at": -1 })
        .limit(50)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut notifications = Vec::new();

    while let Some(result) = cursor.next().await {
        let n = match result {
            Ok(n) => n,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        // Resolved here rather than by the client so the list arrives with the
        // blur hash and dimensions already attached — the row can be drawn at
        // its final size before the thumbnail loads.
        let image = if let Some(ref img_uuid) = n.image_id {
            db.collection::<ImageStruct>("image")
                .find_one(doc!{ "uuid": img_uuid }).await.ok().flatten()
        } else {
            None
        };

        notifications.push(json!({
            "uuid": n.uuid,
            "type": n.r#type,
            "title": n.title,
            "body": n.body,
            "image": image,
            "related_id": n.related_id,
            "link": n.link,
            "is_read": n.is_read,
            "created_at": n.created_at,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(notifications))
}
