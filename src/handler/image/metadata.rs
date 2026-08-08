use mongodb::bson::doc;
use crate::model::ImageStruct;
use crate::builtins::mongo::MongoDB;
use crate::utils::response::Response;
use actix_web::{web, Error, HttpResponse};

pub async fn task(image_id: web::Path<String>) -> Result<HttpResponse, Error> {
    let image_id = image_id.trim().to_string();

    let db = MongoDB.connect();
    let collection = db.collection::<ImageStruct>("image");

    let result = collection.find_one(doc!{
        "uuid": image_id
    }).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::not_found("Image not found!"));
    }

    let image_data = option.unwrap();

    Ok(
        HttpResponse::Ok()
        .content_type("application/json")
        .json(image_data)
    )
}