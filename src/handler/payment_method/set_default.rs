use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Payment::PaymentMethod;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<PaymentMethod>("payment_method");
    let method_id = path.into_inner();

    let owns = col
        .find_one(doc! { "uuid": &method_id, "user_id": &user.user_id, "deleted_at": null })
        .await
        .ok()
        .flatten();
    if owns.is_none() {
        return Ok(Response::not_found("Payment method not found"));
    }

    col.update_many(
        doc! { "user_id": &user.user_id, "deleted_at": null },
        doc! { "$set": { "is_default": false } },
    )
    .await
    .ok();

    let result = col.update_one(doc! { "uuid": &method_id }, doc! { "$set": { "is_default": true } }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("Default payment method updated"))
}
