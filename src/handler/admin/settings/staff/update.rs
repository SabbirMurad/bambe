use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Store::{StaffMember, StaffRole, StaffStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    role: Option<StaffRole>,
    status: Option<StaffStatus>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Staff).await?;
    let db = MongoDB.connect();
    let col = db.collection::<StaffMember>("staff_member");
    let staff_id = path.into_inner();

    let member = match col.find_one(doc! { "uuid": &staff_id }).await.ok().flatten() {
        Some(m) => m,
        None => return Ok(Response::not_found("Staff member not found")),
    };
    /* The last active Owner can't be demoted or suspended: nobody would be
       left able to manage staff. (access.rs would hand the role to the
       next admin to sign in, but that should be a choice, not an accident.) */
    let stays_owner = body.role.as_ref().map(|r| *r == StaffRole::Owner).unwrap_or(member.role == StaffRole::Owner)
        && body.status.as_ref().map(|s| *s != StaffStatus::Suspended).unwrap_or(member.status != StaffStatus::Suspended);
    if member.role == StaffRole::Owner && member.status != StaffStatus::Suspended && !stays_owner {
        let other_owners = col
            .count_documents(doc! { "role": "Owner", "status": { "$ne": "Suspended" }, "uuid": { "$ne": &staff_id } })
            .await
            .unwrap_or(0);
        if other_owners == 0 {
            return Ok(Response::bad_request("This is the only Owner. Make someone else an Owner first."));
        }
    }

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
