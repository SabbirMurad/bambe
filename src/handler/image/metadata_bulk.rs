use futures::StreamExt;
use mongodb::bson::doc;
use crate::model::ImageStruct;
use crate::builtins::mongo::MongoDB;
use crate::utils::response::Response;
use actix_web::{web, Error, HttpResponse};

pub async fn task(image_ids: web::Json<Vec<String>>) -> Result<HttpResponse, Error> {
    let image_ids = image_ids.clone();

    let db = MongoDB.connect();
    let collection = db.collection::<ImageStruct>("image");

    let result = collection.find(doc!{
        "uuid": {"$in": image_ids}
    }).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();

    let mut images = Vec::new();
    while let Some(result) = cursor.next().await {
        if let Err(error) = result {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        let image_data = result.unwrap();

        images.push(image_data);
    }

    Ok(
        HttpResponse::Ok()
        .content_type("application/json")
        .json(images)
    )
}