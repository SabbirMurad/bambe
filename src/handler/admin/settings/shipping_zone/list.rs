use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Store::ShippingZone;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();

    let result = db.collection::<ShippingZone>("shipping_zone").find(doc! {}).sort(doc! { "position": 1 }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut zones = Vec::new();
    while let Some(result) = cursor.next().await {
        let z = match result {
            Ok(z) => z,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        zones.push(z);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(zones))
}
