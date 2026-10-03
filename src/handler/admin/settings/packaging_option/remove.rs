use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::PackagingOption;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let id = path.into_inner();

    

    let result = db.collection::<PackagingOption>("packaging_option").delete_one(doc! { "uuid": &id }).await;
    match result {
        Ok(r) if r.deleted_count == 0 => Ok(Response::not_found("Packaging option not found")),
        Ok(_) => Ok(Response::ok_message("Packaging option deleted")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
