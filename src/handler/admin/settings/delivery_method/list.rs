use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::DeliveryMethod;

/// Every row, enabled or not — the console has to be able to re-enable one it
/// turned off. The storefront's own list filters on `enabled`.
pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let result = db
        .collection::<DeliveryMethod>("delivery_method")
        .find(doc! {})
        .sort(doc! { "position": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut rows = Vec::new();
    while let Some(next) = cursor.next().await {
        match next {
            Ok(row) => rows.push(row),
            Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(rows))
}
