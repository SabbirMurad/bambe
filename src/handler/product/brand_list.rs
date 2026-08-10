use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::model::Product::Brand;

pub async fn task() -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let result = db
        .collection::<Brand>("brand")
        .find(doc! {})
        .sort(doc! { "name": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut brands = Vec::new();
    while let Some(Ok(b)) = cursor.next().await {
        brands.push(b);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(brands))
}
