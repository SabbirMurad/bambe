use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Store::StaffMember;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let result = db.collection::<StaffMember>("staff_member").find(doc! {}).sort(doc! { "invited_at": -1 }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut staff = Vec::new();
    while let Some(result) = cursor.next().await {
        let s = match result {
            Ok(s) => s,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        staff.push(s);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(staff))
}
