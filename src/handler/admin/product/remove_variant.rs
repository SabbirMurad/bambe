use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Product::ProductVariant;
use super::recompute_aggregates;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<ProductVariant>("product_variant");
    let variant_id = path.into_inner();

    let existing = col.find_one(doc! { "uuid": &variant_id }).await.ok().flatten();
    let existing = match existing {
        Some(v) => v,
        None => return Ok(Response::not_found("Variant not found")),
    };

    if let Err(error) = col.delete_one(doc! { "uuid": &variant_id }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    recompute_aggregates(&db, &existing.product_id).await;

    Ok(Response::ok_message("Variant deleted"))
}
