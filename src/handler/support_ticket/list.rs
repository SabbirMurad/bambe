use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Support::SupportTicket;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let result = db
        .collection::<SupportTicket>("support_ticket")
        .find(doc! { "user_id": &user.user_id })
        .sort(doc! { "last_message_at": -1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut tickets = Vec::new();
    while let Some(result) = cursor.next().await {
        let t = match result {
            Ok(t) => t,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        tickets.push(t);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(tickets))
}
