use futures::StreamExt;
use mongodb::bson::doc;
use serde_json::json;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Support::{SupportTicket, TicketMessage};

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let ticket_id = path.into_inner();

    let ticket = db
        .collection::<SupportTicket>("support_ticket")
        .find_one(doc! { "uuid": &ticket_id, "user_id": &user.user_id })
        .await
        .ok()
        .flatten();
    let ticket = match ticket {
        Some(t) => t,
        None => return Ok(Response::not_found("Ticket not found")),
    };

    // Internal staff notes never leave the console.
    let result = db
        .collection::<TicketMessage>("ticket_message")
        .find(doc! { "ticket_id": &ticket_id, "is_internal": false })
        .sort(doc! { "created_at": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut messages = Vec::new();
    while let Some(Ok(m)) = cursor.next().await {
        messages.push(m);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "ticket": ticket,
        "messages": messages,
    })))
}
