use crate::Model;
use mongodb::bson::doc;
use crate::builtins::mongo::MongoDB;
use actix_web::{ Error, HttpResponse};
use crate::utils::response::Response;

pub async fn task() -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let collection = db.collection::<Model::Metadata::AppMetadata>("app_metadata");
    
    let result = collection.find_one(
        doc!{},
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::not_found("App metadata not found"));
    }

    let app_info = option.unwrap();

    Ok(HttpResponse::Ok().content_type("application/json").json(app_info))
}