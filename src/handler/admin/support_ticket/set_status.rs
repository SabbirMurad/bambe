use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Support::{SupportTicket, TicketStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    status: TicketStatus,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Support).await?;
    let db = MongoDB.connect();
    let ticket_id = path.into_inner();
    let now = Utc::now().timestamp_millis();

    let mut set_doc = doc! {};
    set_doc.insert("status", mongodb::bson::to_bson(&body.status).unwrap());
    match body.status {
        TicketStatus::Resolved => { set_doc.insert("resolved_at", now); }
        TicketStatus::Closed => { set_doc.insert("closed_at", now); }
        _ => {}
    }

    let result = db
        .collection::<SupportTicket>("support_ticket")
        .update_one(doc! { "uuid": &ticket_id }, doc! { "$set": set_doc })
        .await;

    match result {
        Ok(r) if r.matched_count == 0 => Ok(Response::not_found("Ticket not found")),
        Ok(_) => Ok(Response::ok_message("Ticket status updated")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
