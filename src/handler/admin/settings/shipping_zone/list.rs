use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Store::ShippingZone;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let result = db.collection::<ShippingZone>("shipping_zone").find(doc! {}).sort(doc! { "position": 1 }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut zones = Vec::new();
    while let Some(Ok(z)) = cursor.next().await {
        zones.push(z);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(zones))
}
