use mongodb::bson::doc;
use crate::model::ImageStruct;
use crate::utils::response::Response;
use crate::builtins::{mongo::MongoDB, sqlite};
use actix_web::{web, Error, HttpResponse};

pub async fn task(image_id: web::Path<String>) -> Result<HttpResponse, Error> {
    let image_id = image_id.trim().to_string();
    let sqlite_conn = sqlite::connect(sqlite::DBF::IMG).unwrap();

    let result = sqlite_conn.query_row(
        "SELECT original FROM image WHERE uuid = ?1",
        [&image_id],
        |row| row.get::<usize, Vec<u8>>(0)
    );

    let image_data = match result {
        Ok(data) => data,
        Err(error) => return Ok(
            Response::internal_server_error(&error.to_string())
        )
    };

    let db = MongoDB.connect();
    let collection = db.collection::<ImageStruct>("image");
    let result = collection.find_one(doc!{"uuid": image_id}).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::not_found("Image not found!"));
    }

    let image_meta = option.unwrap();

    if image_meta.temporary  || image_meta.deleted {
        return Ok(Response::not_found("Image not found!"));
    }
    Ok(
        HttpResponse::Ok()
        .content_type("image/png")
        .body(image_data)
    )
}