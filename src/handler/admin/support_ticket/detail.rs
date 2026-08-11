use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Support::{SupportTicket, TicketMessage};

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let ticket_id = path.into_inner();

    let ticket = db.collection::<SupportTicket>("support_ticket").find_one(doc! { "uuid": &ticket_id }).await.ok().flatten();
    let ticket = match ticket {
        Some(t) => t,
        None => return Ok(Response::not_found("Ticket not found")),
    };

    // Opening the thread in the console is what "read" means here.
    if ticket.unread_by_staff {
        db.collection::<SupportTicket>("support_ticket")
            .update_one(doc! { "uuid": &ticket_id }, doc! { "$set": { "unread_by_staff": false } })
            .await
            .ok();
    }

    let result = db.collection::<TicketMessage>("ticket_message").find(doc! { "ticket_id": &ticket_id }).sort(doc! { "created_at": 1 }).await;
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
