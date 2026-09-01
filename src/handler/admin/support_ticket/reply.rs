use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
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

    /*
      An internal note touches none of the ticket's rollup fields.

      `detail.rs` filters `is_internal` out of the thread it hands the
      customer, but `list.rs` returns the whole SupportTicket document —
      preview included — so writing the note into `last_message_preview`
      put the first 140 characters of a staff-only note straight into the
      customer's own ticket list. `message_count` is the same story: it is
      what the customer's thread claims to hold, and counting messages they
      cannot see makes it wrong.
    */
    let mut set_doc = Document::new();
    let mut inc_doc = Document::new();

    if !body.is_internal {
        set_doc.insert("last_message_preview", preview(&body.body));
        set_doc.insert("last_message_at", now);
        inc_doc.insert("message_count", 1);

        // A customer-facing reply is what "Pending — waiting on the
        // customer" means; an internal note doesn't move the conversation
        // forward.
        if !matches!(ticket.status, TicketStatus::Closed) {
            set_doc.insert("status", "Pending");
        }
    }

    if set_doc.is_empty() && inc_doc.is_empty() {
        return Ok(HttpResponse::Ok().content_type("application/json").json(message));
    }

    let mut update = Document::new();
    if !set_doc.is_empty() { update.insert("$set", set_doc); }
    if !inc_doc.is_empty() { update.insert("$inc", inc_doc); }

    let result = db
        .collection::<SupportTicket>("support_ticket")
        .update_one(doc! { "uuid": &ticket_id }, update)
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // Only a customer-facing reply reaches the customer. The internal-note
    // path returns above, before this.
    // `user_id` is optional — a ticket can be opened by a guest, who has no
    // account to hang a notification on.
    if let Some(user_id) = &ticket.user_id {
        crate::utils::notify::send(
            &db,
            user_id,
            crate::utils::notify::Notice {
                kind: crate::model::Notification::NotifType::SupportReply,
                title: format!("Reply on ticket {}", ticket.number),
                body: preview(&body.body),
                image_id: None,
                related_id: Some(ticket_id.clone()),
                link: Some(format!("/account/support?id={}", ticket_id)),
                // The thread lives behind a login and can hold personal detail, so
                // the email says there's a reply rather than quoting it.
                email: None,
            },
        ).await;
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(message))
}
