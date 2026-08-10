use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::{AccountRole, AdminAccount};
use crate::model::Support::{SupportTicket, TicketMessage, TicketStatus};

fn preview(body: &str) -> String {
    body.chars().take(140).collect()
}

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    body: String,
    #[serde(default)]
    image_ids: Vec<String>,
    // A note for the team, never shown to the customer.
    #[serde(default)]
    is_internal: bool,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let ticket_id = path.into_inner();

    if body.body.trim().is_empty() {
        return Ok(Response::bad_request("Message can't be empty"));
    }

    let ticket = db.collection::<SupportTicket>("support_ticket").find_one(doc! { "uuid": &ticket_id }).await.ok().flatten();
    let ticket = match ticket {
        Some(t) => t,
        None => return Ok(Response::not_found("Ticket not found")),
    };

    let admin_name = db
        .collection::<AdminAccount>("admin_account")
        .find_one(doc! { "uuid": &admin.user_id })
        .await
        .ok()
        .flatten()
        .map(|a| if a.name.is_empty() { "Support".to_string() } else { a.name })
        .unwrap_or_else(|| "Support".to_string());

    let now = Utc::now().timestamp_millis();
    let message = TicketMessage {
        uuid: Uuid::now_v7().to_string(),
        ticket_id: ticket_id.clone(),
        from_staff: true,
        author_id: Some(admin.user_id),
        author_name: admin_name,
        body: body.body.clone(),
        image_ids: body.image_ids.clone(),
        is_internal: body.is_internal,
        created_at: now,
    };

    if let Err(error) = db.collection::<TicketMessage>("ticket_message").insert_one(&message).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut set_doc = doc! {
        "last_message_preview": preview(&body.body),
        "last_message_at": now,
    };
    // A customer-facing reply is what "Pending — waiting on the customer"
    // means; an internal note doesn't move the conversation forward.
    if !body.is_internal && !matches!(ticket.status, TicketStatus::Closed) {
        set_doc.insert("status", "Pending");
    }

    let result = db
        .collection::<SupportTicket>("support_ticket")
        .update_one(doc! { "uuid": &ticket_id }, doc! { "$set": set_doc, "$inc": { "message_count": 1 } })
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(message))
}
