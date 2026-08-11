use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::model::Order::PackagingOption;

pub async fn task() -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let result = db
        .collection::<PackagingOption>("packaging_option")
        .find(doc! { "enabled": true })
        .sort(doc! { "position": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut options = Vec::new();
    while let Some(Ok(o)) = cursor.next().await {
        options.push(o);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(options))
}
