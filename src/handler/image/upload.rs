use uuid::Uuid;
use chrono::Utc;
use futures_util::StreamExt as _;
use webp::Encoder;
use std::collections::HashMap;
use actix_multipart::Multipart;
use crate::util::{blurhash, response::Response};
use actix_web::{Error, HttpRequest, HttpResponse};
use image::io::Reader as ImageReader;
use image::DynamicImage;
use crate::builtins::{mongo::MongoDB, sqlite};
use crate::model::{AllowedImageType, ImageStruct, AssetUsedAt};
use crate::Middleware::Auth::{require_access, AccessRequirement};

/*
  A decoded image costs width * height * 4 bytes of memory, which the 64 MB
  payload cap does not bound — a 20000 x 20000 PNG of flat colour compresses
  to a couple of megabytes and expands to 1.6 GB. Probed from the file header
  before anything is decoded.
*/
const MAX_PIXELS: u64 = 40_000_000;

/* One request writes one product's gallery, not a bulk import. */
const MAX_IMAGES_PER_REQUEST: usize = 20;

/*
  What the blur placeholder is derived from. A blurhash is a handful of
  cosine terms; running the transform over full resolution costs seconds per
  image and produces the same string a thumbnail does.
*/
const BLUR_SOURCE_MAX: u32 = 64;

/* One validated image, held until every image in the request has passed. */
struct PendingImage {
    uuid: String,
    original: Vec<u8>,
    webp: Vec<u8>,
    doc: ImageStruct,
}

pub async fn task(req: HttpRequest, mut payload: Multipart) -> Result<HttpResponse, Error> {
    // Uploads must be authenticated so anonymous callers can't fill storage.
    require_access(&req, AccessRequirement::AnyToken)?;

    let mut images_data: Vec<(String, Vec<u8>)> = Vec::new();
    let mut text_fields: HashMap<String, String> = HashMap::new();

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
                if images_data.len() >= MAX_IMAGES_PER_REQUEST {
                    return Ok(Response::bad_request(&format!(
                        "At most {} images per request",
                        MAX_IMAGES_PER_REQUEST
                    )));
                }

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

    if images_data.is_empty() {
        return Ok(Response::bad_request(
            "No file was attached. Send each image as a file field named \
             `image_0`, `image_1`, ... with its metadata alongside it."
        ));
    }

    let created_at = Utc::now().timestamp_millis();

    /*
      Everything is validated and converted before a single byte is written.
      The old version wrote as it went, so the fourth image of a gallery
      failing left three half-attributed rows behind — and because every
      lookup was an `.unwrap()`, a missing field killed the worker thread and
      dropped the connection instead of answering 400.
    */
    let mut pending: Vec<PendingImage> = Vec::with_capacity(images_data.len());

    for (position, (field_name, bytes)) in images_data.iter().enumerate() {
        let index = field_index(field_name).unwrap_or(position);

        if bytes.is_empty() {
            return Ok(Response::bad_request(&format!(
                "`{}` is empty", field_name
            )));
        }

        // Which slot the image is claimed by. Required: an unattributed
        // image is a file nothing will ever delete.
        let used_at = match text_fields.get(&format!("used_at_{}", index)) {
            Some(value) => match AssetUsedAt::parse(value) {
                Some(used_at) => used_at,
                None => return Ok(Response::bad_request(&format!(
                    "`used_at_{}` is `{}`, which is not a known slot. One of: {}",
                    index, value, AssetUsedAt::variants()
                ))),
            },
            None => return Ok(Response::bad_request(&format!(
                "`used_at_{}` is required alongside `{}`. One of: {}",
                index, field_name, AssetUsedAt::variants()
            ))),
        };

        // Optional, and false is the safe default: a permanent image is
        // merely unreferenced, a temporary one gets swept.
        let temporary = match text_fields.get(&format!("temporary_{}", index)) {
            None => false,
            Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => return Ok(Response::bad_request(&format!(
                    "`temporary_{}` must be true or false, got `{}`",
                    index, value
                ))),
            },
        };

        let image_type = match imghdr::from_bytes(bytes) {
            Some(image_type) => match image_type {
                imghdr::Type::Gif => AllowedImageType::Gif,
                imghdr::Type::Png => AllowedImageType::Png,
                imghdr::Type::Jpeg => AllowedImageType::Jpeg,
                imghdr::Type::Webp => AllowedImageType::Webp,
                other => {
                    return Ok(Response::bad_request(&format!(
                        "`{}` is {:?}; only gif, png, jpeg and webp are accepted",
                        field_name, other
                    )));
                }
            },
            None => {
                return Ok(Response::bad_request(&format!(
                    "`{}` is not an image", field_name
                )));
            },
        };

        // Read from the header, so an oversized image is refused before it
        // is expanded into memory.
        let (width, height) = match imagesize::blob_size(bytes) {
            Ok(size) => (size.width, size.height),
            Err(error) => {
                log::error!("{:?}", error);
                return Ok(Response::bad_request(&format!(
                    "`{}` has unreadable dimensions", field_name
                )));
            },
        };

        if width as u64 * height as u64 > MAX_PIXELS {
            return Ok(Response::bad_request(&format!(
                "`{}` is {}x{}, over the {} megapixel limit",
                field_name, width, height, MAX_PIXELS / 1_000_000
            )));
        }

        let decoded = match decode(bytes) {
            Ok(decoded) => decoded,
            Err(error) => {
                log::error!("{:?}", error);
                return Ok(Response::bad_request(&format!(
                    "`{}` could not be decoded: {}", field_name, error
                )));
            },
        };

        let webp_bytes = match convert_to_webp(&decoded) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(Response::internal_server_error(&error));
            },
        };

        // Derived, never taken from the client — see utils::blurhash.
        let blur_hash = blur_hash_of(&decoded);

        let uuid = Uuid::now_v7().to_string();

        pending.push(PendingImage {
            uuid: uuid.clone(),
            doc: ImageStruct {
                uuid,
                blur_hash,
                width,
                height,
                created_at,
                original_size: bytes.len(),
                webp_size: webp_bytes.len(),
                used_at,
                temporary,
                deleted: false,
                original_type: image_type.to_str().to_string(),
            },
            original: bytes.clone(),
            webp: webp_bytes,
        });
    }

    let sqlite_conn = match sqlite::connect(sqlite::DBF::IMG) {
        Ok(conn) => conn,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        },
    };

    let db = MongoDB.connect();
    let collection = db.collection::<ImageStruct>("image");
    let mut image_ids: Vec<String> = Vec::with_capacity(pending.len());

    for item in pending.iter() {
        /*
          Blob first. Metadata pointing at bytes that were never stored is a
          broken image on every page that references it, while a blob with no
          metadata is invisible — and is deleted below if the metadata write
          is what failed.
        */
        let result = sqlite_conn.execute("
            INSERT INTO image (uuid, original, webp)
            VALUES (?1, ?2, ?3)",
            (
                &item.uuid,
                &item.original,
                &item.webp
            )
        );

        if let Err(error) = result {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        if let Err(error) = collection.insert_one(item.doc.clone()).await {
            log::error!("{:?}", error);
            let _ = sqlite_conn.execute(
                "DELETE FROM image WHERE uuid = ?1",
                (&item.uuid,)
            );

            return Ok(Response::internal_server_error(&error.to_string()));
        }

        image_ids.push(item.uuid.clone());
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(image_ids))
}

/*
  `image_0` names the metadata that belongs to it — `used_at_0`, and so on.
  The old version did `split('_').nth(1).unwrap().parse().unwrap()`, so a
  field called `image` panicked, and `product_image_0` read "image" and
  panicked too. Take the digits after the last underscore, and fall back to
  the file's position in the request when there are none.
*/
fn field_index(field_name: &str) -> Option<usize> {
    field_name.rsplit('_').next()?.parse().ok()
}

fn decode(image_bytes: &[u8]) -> Result<DynamicImage, String> {
    // Decode the image (autodetects format)
    let reader = match ImageReader::new(std::io::Cursor::new(image_bytes))
    .with_guessed_format() {
        Ok(reader) => reader,
        Err(error) => return Err(error.to_string()),
    };

    match reader.decode() {
        Ok(img) => Ok(img),
        Err(error) => Err(error.to_string()),
    }
}

fn convert_to_webp(img: &DynamicImage) -> Result<Vec<u8>, String> {
    // Convert to RGBA (preserves transparency)
    let rgba = img.to_rgba8();

    // Encode to WebP with quality 80
    let encoder = Encoder::from_rgba(&rgba, img.width(), img.height());
    let webp = encoder.encode(80.0);

    Ok(webp.to_vec())
}

fn blur_hash_of(img: &DynamicImage) -> String {
    let small = img.thumbnail(BLUR_SOURCE_MAX, BLUR_SOURCE_MAX).to_rgb8();

    // An empty hash is a missing placeholder, not a failed upload — the
    // photo itself is fine, so it is not worth refusing the request over.
    blurhash::encode(
        4,
        3,
        small.width() as usize,
        small.height() as usize,
        small.as_raw(),
    ).unwrap_or_default()
}

/*
  Stores one image the server fetched itself rather than one a client posted
  - the profile photo a Google or Facebook sign-in carries. Same checks and
  same two writes as an upload, so the result is indistinguishable from one
  and is served by /image/webp/{id} like any other.
*/
pub async fn store(bytes: &[u8], used_at: AssetUsedAt) -> Result<String, String> {
    let image_type = match imghdr::from_bytes(bytes) {
        Some(imghdr::Type::Gif) => AllowedImageType::Gif,
        Some(imghdr::Type::Png) => AllowedImageType::Png,
        Some(imghdr::Type::Jpeg) => AllowedImageType::Jpeg,
        Some(imghdr::Type::Webp) => AllowedImageType::Webp,
        other => return Err(format!("not an accepted image type: {:?}", other)),
    };

    let (width, height) = imagesize::blob_size(bytes)
        .map(|size| (size.width, size.height))
        .map_err(|error| format!("unreadable dimensions: {:?}", error))?;
    if width as u64 * height as u64 > MAX_PIXELS {
        return Err(format!("{}x{} is over the pixel limit", width, height));
    }

    let decoded = decode(bytes)?;
    let webp = convert_to_webp(&decoded)?;
    let uuid = Uuid::now_v7().to_string();
    let doc = ImageStruct {
        uuid: uuid.clone(),
        blur_hash: blur_hash_of(&decoded),
        width,
        height,
        created_at: Utc::now().timestamp_millis(),
        original_size: bytes.len(),
        webp_size: webp.len(),
        used_at,
        temporary: false,
        deleted: false,
        original_type: image_type.to_str().to_string(),
    };

    let sqlite_conn = sqlite::connect(sqlite::DBF::IMG).map_err(|error| error.to_string())?;
    sqlite_conn
        .execute("INSERT INTO image (uuid, original, webp) VALUES (?1, ?2, ?3)", (&uuid, bytes, &webp))
        .map_err(|error| error.to_string())?;

    if let Err(error) = MongoDB.connect().collection::<ImageStruct>("image").insert_one(doc).await {
        let _ = sqlite_conn.execute("DELETE FROM image WHERE uuid = ?1", (&uuid,));
        return Err(error.to_string());
    }
    Ok(uuid)
}

/* Removes an image `store` wrote, once nothing points at it any more. */
pub async fn discard(uuid: &str) {
    if let Ok(conn) = sqlite::connect(sqlite::DBF::IMG) {
        let _ = conn.execute("DELETE FROM image WHERE uuid = ?1", (uuid,));
    }
    let _ = MongoDB.connect()
        .collection::<ImageStruct>("image")
        .delete_one(mongodb::bson::doc! { "uuid": uuid })
        .await;
}
