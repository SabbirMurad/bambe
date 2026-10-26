use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Support::SupportTicket;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    // None unassigns the ticket.
    admin_id: Option<String>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Support).await?;
    let db = MongoDB.connect();

    let value = match &body.admin_id {
        Some(id) => mongodb::bson::Bson::String(id.clone()),
        None => mongodb::bson::Bson::Null,
    };

    let result = db
        .collection::<SupportTicket>("support_ticket")
        .update_one(doc! { "uuid": path.into_inner() }, doc! { "$set": { "assigned_to": value } })
        .await;

    match result {
        Ok(r) if r.matched_count == 0 => Ok(Response::not_found("Ticket not found")),
        Ok(_) => Ok(Response::ok_message("Ticket assigned")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
