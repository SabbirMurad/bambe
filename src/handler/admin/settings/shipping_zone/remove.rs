use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Store::ShippingZone;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let result = db.collection::<ShippingZone>("shipping_zone").delete_one(doc! { "uuid": path.into_inner() }).await;
    match result {
        Ok(r) if r.deleted_count == 0 => Ok(Response::not_found("Shipping zone not found")),
        Ok(_) => Ok(Response::ok_message("Shipping zone deleted")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
