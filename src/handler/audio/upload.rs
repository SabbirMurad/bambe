use uuid::Uuid;
use chrono::Utc;
use futures_util::StreamExt as _;
use std::collections::HashMap;
use actix_multipart::Multipart;
use crate::utils::response::Response;
use actix_web::{Error, HttpRequest, HttpResponse};
use crate::builtins::{mongo::MongoDB, sqlite};
use crate::model::{AudioStruct, AssetUsedAt};
use crate::Middleware::Auth::{require_access, AccessRequirement};

/// Accepts a single audio file (voice note) plus its metadata as multipart form
/// data, stores the bytes in sqlite and the metadata in mongo, and returns the
/// created [AudioStruct] so the client can attach it to a chat message.
///
/// Expected fields: one file field (the audio) + text fields `length` (ms),
/// `used_at`, `temporary`, and optionally `uuid`.
pub async fn task(req: HttpRequest, mut payload: Multipart) -> Result<HttpResponse, Error> {
    // Uploads must be authenticated so anonymous callers can't fill storage.
    require_access(&req, AccessRequirement::AnyToken)?;

    let mut audio_bytes: Option<Vec<u8>> = None;
    let mut mime = String::from("audio/mp4");
    let mut text_fields: HashMap<String, String> = HashMap::new();

    while let Some(item) = payload.next().await {
        let mut field = item?;

        let (field_name, file_name) = {
            let cd = match field.content_disposition() {
                Some(cd) => cd,
                None => return Ok(Response::bad_request("Missing content disposition")),
            };

            let field_name = cd.get_name().map(|s| s.to_string());
            let file_name = cd.get_filename().map(|s| s.to_string());

            if field_name.is_none() {
                return Ok(Response::bad_request("Missing field name"));
            }

            (field_name.unwrap(), file_name)
        };

        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            bytes.extend_from_slice(&data);
        }

        match file_name {
            Some(name) => {
                mime = mime_from_name(&name);
                audio_bytes = Some(bytes);
            }
            None => {
                text_fields.insert(
                    field_name,
                    String::from_utf8_lossy(&bytes).to_string(),
                );
            }
        };
    }

    let bytes = match audio_bytes {
        Some(bytes) if !bytes.is_empty() => bytes,
        _ => return Ok(Response::bad_request("Missing audio file")),
    };

    let created_uuid = Uuid::now_v7().to_string();
    let uuid = text_fields.get("uuid").cloned().unwrap_or(created_uuid);
    let length: i64 = text_fields
        .get("length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let used_at = text_fields
        .get("used_at")
        .map(|s| s.as_str())
        .unwrap_or("Chat");
    let temporary: bool = text_fields
        .get("temporary")
        .and_then(|v| v.parse().ok())
        .unwrap_or(false);

    let audio_doc = AudioStruct {
        uuid: uuid.clone(),
        length,
        size: bytes.len(),
        used_at: AssetUsedAt::from_str(used_at),
        created_at: Utc::now().timestamp_millis(),
        temporary,
        deleted: false,
    };

    // Metadata in mongo
    let db = MongoDB.connect();
    let collection = db.collection::<AudioStruct>("audio");
    if let Err(error) = collection.insert_one(audio_doc.clone()).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // Bytes in sqlite (same DB file as images)
    let sqlite_conn = sqlite::connect(sqlite::DBF::IMG).unwrap();
    let result = sqlite_conn.execute(
        "INSERT INTO audio (uuid, data, mime) VALUES (?1, ?2, ?3)",
        (&uuid, &bytes, &mime),
    );
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(audio_doc))
}

fn mime_from_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.ends_with(".aac") {
        "audio/aac".to_string()
    } else if lower.ends_with(".mp3") {
        "audio/mpeg".to_string()
    } else if lower.ends_with(".ogg") || lower.ends_with(".opus") {
        "audio/ogg".to_string()
    } else if lower.ends_with(".wav") {
        "audio/wav".to_string()
    } else {
        // .m4a / .mp4 / aac-in-mp4 container
        "audio/mp4".to_string()
    }
}
