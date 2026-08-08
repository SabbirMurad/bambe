use mongodb::bson::doc;
use crate::model::AudioStruct;
use crate::utils::response::Response;
use crate::builtins::{mongo::MongoDB, sqlite};
use actix_web::{web, Error, HttpResponse};

/// Streams a stored voice note back to the client, with the mime type it was
/// uploaded with, so `audioplayers` can play it directly from the URL.
pub async fn task(audio_id: web::Path<String>) -> Result<HttpResponse, Error> {
    let audio_id = audio_id.trim().to_string();
    let sqlite_conn = sqlite::connect(sqlite::DBF::IMG).unwrap();

    let result = sqlite_conn.query_row(
        "SELECT data, mime FROM audio WHERE uuid = ?1",
        [&audio_id],
        |row| Ok((
            row.get::<usize, Vec<u8>>(0)?,
            row.get::<usize, String>(1)?,
        )),
    );

    let (audio_data, mime) = match result {
        Ok(data) => data,
        Err(_) => return Ok(Response::not_found("Audio not found!")),
    };

    let db = MongoDB.connect();
    let collection = db.collection::<AudioStruct>("audio");
    let result = collection.find_one(doc!{"uuid": &audio_id}).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if option.is_none() {
        return Ok(Response::not_found("Audio not found!"));
    }

    let meta = option.unwrap();
    if meta.deleted {
        return Ok(Response::not_found("Audio not found!"));
    }

    Ok(HttpResponse::Ok().content_type(mime).body(audio_data))
}
