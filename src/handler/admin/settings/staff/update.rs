use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Store::{StaffMember, StaffRole, StaffStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    role: Option<StaffRole>,
    status: Option<StaffStatus>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<StaffMember>("staff_member");
    let staff_id = path.into_inner();

    let mut set_doc = Document::new();
    if let Some(v) = &body.role { set_doc.insert("role", mongodb::bson::to_bson(v).unwrap()); }
    if let Some(v) = &body.status { set_doc.insert("status", mongodb::bson::to_bson(v).unwrap()); }

    if set_doc.is_empty() {
        return Ok(Response::bad_request("Nothing to update"));
    }

    let result = col.update_one(doc! { "uuid": &staff_id }, doc! { "$set": set_doc }).await;
    match result {
        Ok(r) if r.matched_count == 0 => Ok(Response::not_found("Staff member not found")),
        Ok(_) => {
            let updated = col.find_one(doc! { "uuid": &staff_id }).await.ok().flatten();
            Ok(HttpResponse::Ok().content_type("application/json").json(updated))
        }
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
