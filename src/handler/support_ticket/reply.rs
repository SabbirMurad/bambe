use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Support::{SupportTicket, TicketMessage, TicketStatus};

fn preview(body: &str) -> String {
    body.chars().take(140).collect()
}

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    body: String,
    #[serde(default)]
    image_ids: Vec<String>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let ticket_id = path.into_inner();

    if body.body.trim().is_empty() {
        return Ok(Response::bad_request("Message can't be empty"));
    }

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

    if ticket.status == TicketStatus::Closed {
        return Ok(Response::bad_request("This ticket is closed"));
    }

    let now = Utc::now().timestamp_millis();
    let message = TicketMessage {
        uuid: Uuid::now_v7().to_string(),
        ticket_id: ticket_id.clone(),
        from_staff: false,
        author_id: Some(user.user_id),
        author_name: ticket.name.clone(),
        body: body.body.clone(),
        image_ids: body.image_ids.clone(),
        is_internal: false,
        created_at: now,
    };

    if let Err(error) = db.collection::<TicketMessage>("ticket_message").insert_one(&message).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // A customer reply always needs a staff response next, even if the
    // thread had been marked Pending (staff replied) or Resolved.
    let result = db
        .collection::<SupportTicket>("support_ticket")
        .update_one(
            doc! { "uuid": &ticket_id },
            doc! {
                "$set": {
                    "last_message_preview": preview(&body.body),
                    "last_message_at": now,
                    "unread_by_staff": true,
                    "status": "Open",
                },
                "$inc": { "message_count": 1 },
            },
        )
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(message))
}
