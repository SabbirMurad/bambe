use uuid::Uuid;
use chrono::Utc;
use futures_util::StreamExt as _;
use webp::Encoder;
use std::collections::HashMap;
use actix_multipart::Multipart;
use crate::utils::response::Response;
use actix_web::{Error, HttpRequest, HttpResponse};
use image::io::Reader as ImageReader;
use crate::builtins::{mongo::MongoDB, sqlite};
use crate::model::{AllowedImageType, ImageStruct, AssetUsedAt};
use crate::Middleware::Auth::{require_access, AccessRequirement};


pub async fn task(req: HttpRequest, mut payload: Multipart) -> Result<HttpResponse, Error> {
    // Uploads must be authenticated so anonymous callers can't fill storage.
    require_access(&req, AccessRequirement::AnyToken)?;

    let mut images_data: Vec<(String, Vec<u8>)> = Vec::new();
    let mut text_fields: HashMap<String, String> = HashMap::new();
    let mut image_ids: Vec<String> = Vec::new();

    // Iterate over multipart fields
    while let Some(item) = payload.next().await {
        let mut field = item?;

        // You can check the field name if you have multiple fields
        let (field_name, file_name) = {
            let cd = match field.content_disposition() {
                Some(cd) => cd,
                None => return Ok(Response::bad_request(
                    "Missing content disposition"
                )),
            };

            let field_name = cd.get_name().map(|s| s.to_string());
            let file_name = cd.get_filename().map(|s| s.to_string());
            
            if field_name.is_none() {
                return Ok(Response::bad_request(
                    "Missing field name"
                ));
            }

            (field_name.unwrap(), file_name)
        };

        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            bytes.extend_from_slice(&data);
        }

        match file_name {
            Some(_) => {
                images_data.push((field_name.clone(), bytes));
            },
            None => {
                text_fields.insert(
                    field_name.clone(),
                    String::from_utf8_lossy(&bytes).to_string()
                );
            },
        };
    }

    let db = MongoDB.connect();
    let collection = db.collection::<ImageStruct>("image");
    let created_at = Utc::now().timestamp_millis();
    let sqlite_conn = sqlite::connect(sqlite::DBF::IMG).unwrap();

    for (field_name, bytes) in images_data.iter() {
        let index: usize = field_name
            .split('_')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();

        let created_uuid = Uuid::now_v7().to_string();
        let uuid = text_fields
            .get(&format!("uuid_{}", index))
            .unwrap_or(&created_uuid);

        let blur_hash = text_fields
            .get(&format!("blur_hash_{}", index))
            .unwrap();

        let width = text_fields
            .get(&format!("width_{}", index))
            .unwrap();

        let height = text_fields
            .get(&format!("height_{}", index))
            .unwrap();

        let used_at = text_fields
            .get(&format!("used_at_{}", index))
            .unwrap();

        let temporary = text_fields
            .get(&format!("temporary_{}", index))
            .unwrap();

        // Converting to webp
        let webp_bytes = match convert_to_webp(bytes.clone()) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(Response::internal_server_error(&error.to_string()));
            },
        };

        let image_type = match imghdr::from_bytes(bytes) {
            Some(image_type) => match image_type {
                imghdr::Type::Gif => AllowedImageType::Gif,
                imghdr::Type::Png => AllowedImageType::Png,
                imghdr::Type::Jpeg => AllowedImageType::Jpeg,
                imghdr::Type::Webp => AllowedImageType::Webp,
                _ => {
                    return Ok(Response::internal_server_error(
                        "Unsupported image format!"
                    ));
                }
            }, 
            None => {
                return Ok(Response::internal_server_error(
                    "Invalid image format!"
                ));
            },
        };
    
        // Creating the metadata in mongo
        let image_doc = ImageStruct {
            uuid: uuid.clone(),
            blur_hash: blur_hash.clone(),
            width: width.parse().unwrap(),
            height: height.parse().unwrap(),
            created_at,
            original_size: bytes.len(),
            webp_size: webp_bytes.len(),
            used_at: AssetUsedAt::from_str(used_at.as_str()),
            temporary: temporary.parse().unwrap(),
            deleted: false,
            original_type: image_type.to_str().to_string(),
        };

        let result = collection.insert_one(image_doc.clone()).await;
        if let Err(error) = result {
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        // Uploading image to sqlite
        let result = sqlite_conn.execute("
            INSERT INTO image (uuid, original, webp)
            VALUES (?1, ?2, ?3)",
            (
                &uuid,
                &bytes,
                &webp_bytes
            )
        );

        if let Err(error) = result {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        } else {
            image_ids.push(uuid.clone());
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(image_ids))
}

fn convert_to_webp(image_bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    // Decode the image (autodetects format)
    let reader = match ImageReader::new(std::io::Cursor::new(image_bytes))
    .with_guessed_format() {
        Ok(reader) => reader,
        Err(error) => return Err(error.to_string()),
    };

    let img = match reader.decode() {
        Ok(img) => img,
        Err(error) => return Err(error.to_string()),
    };

    // Convert to RGBA (preserves transparency)
    let rgba = img.to_rgba8();

    // Encode to WebP with quality 80
    let encoder = Encoder::from_rgba(&rgba, img.width(), img.height());
    let webp = encoder.encode(80.0);

    Ok(webp.to_vec())
}