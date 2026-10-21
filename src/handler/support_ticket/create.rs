use chrono::Utc;
use uuid::Uuid;
use rand::Rng;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::optional_access;
use crate::model::Support::{SupportTicket, TicketMessage, TicketTopic, TicketStatus, TicketPriority};
use crate::model::Store::StoreSettings;

fn preview(body: &str) -> String {
    body.chars().take(140).collect()
}

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    email: String,
    name: String,
    subject: String,
    topic: TicketTopic,
    body: String,
    order_id: Option<String>,
}

// Open to guests as well as signed-in customers — someone writing in about
// an order they placed without an account still needs a way to reach
// support, and `email` is the only thread back to them in that case.
pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    if body.subject.trim().is_empty() || body.body.trim().is_empty() || body.email.trim().is_empty() {
        return Ok(Response::bad_request("Subject, message and email are required"));
    }

    let db = MongoDB.connect();
    let user = optional_access(&req);
    let now = Utc::now().timestamp_millis();

    let prefix = crate::Handler::Admin::Settings::load(&db)
        .await
        .map(|s| s.ticket_number_prefix)
        .unwrap_or_else(|| "TK".to_string());
    let number = format!("{}-{}", prefix, rand::rng().random_range(1000..9999));

    let ticket = SupportTicket {
        uuid: Uuid::now_v7().to_string(),
        number,
        user_id: user.as_ref().map(|u| u.user_id.clone()),
        email: body.email.clone(),
        name: body.name.clone(),
        subject: body.subject.clone(),
        topic: body.topic.clone(),
        status: TicketStatus::Open,
        priority: TicketPriority::Normal,
        order_id: body.order_id.clone(),
        assigned_to: None,
        last_message_preview: preview(&body.body),
        last_message_at: now,
        unread_by_staff: true,
        message_count: 1,
        created_at: now,
        resolved_at: None,
        closed_at: None,
    };

    if let Err(error) = db.collection::<SupportTicket>("support_ticket").insert_one(&ticket).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let message = TicketMessage {
        uuid: Uuid::now_v7().to_string(),
        ticket_id: ticket.uuid.clone(),
        from_staff: false,
        author_id: user.map(|u| u.user_id),
        author_name: body.name.clone(),
        body: body.body.clone(),
        image_ids: Vec::new(),
        is_internal: false,
        created_at: now,
    };

    if let Err(error) = db.collection::<TicketMessage>("ticket_message").insert_one(&message).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(ticket))
}
