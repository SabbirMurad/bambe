use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::DeliveryMethod;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let id = path.into_inner();

    /*
      Refuse to delete the last enabled method. Checkout cannot quote a price
      without one, so removing it takes the shop offline — and the failure
      shows up as an empty delivery step, not as anything pointing back here.
    */
    let enabled_left = db
        .collection::<DeliveryMethod>("delivery_method")
        .count_documents(doc! { "enabled": true, "uuid": { "$ne": &id } })
        .await
        .unwrap_or(0);
    if enabled_left == 0 {
        return Ok(Response::bad_request(
            "This is the only delivery method customers can choose — add another before removing it",
        ));
    }

    let result = db.collection::<DeliveryMethod>("delivery_method").delete_one(doc! { "uuid": &id }).await;
    match result {
        Ok(r) if r.deleted_count == 0 => Ok(Response::not_found("Delivery method not found")),
        Ok(_) => Ok(Response::ok_message("Delivery method deleted")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
