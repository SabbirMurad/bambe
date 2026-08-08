/*
  if you are using another server to access the images then remove this file
*/

/* CURD - END POINTS FOR IMAGE */
/*
  TODO:
   [1]. Adding large images take a long time due to image processing
        Blur Effect + Thumbnail generation needs to be handled with threads???
   [2]. Png image thumbnail generates black background for alpha channel!
        Can we add custom background color on transparent region???
*/

use uuid::Uuid;
use chrono::Utc;
use crate::builtins::sqlite;
use std::io::Cursor;
use rusqlite::params;
use serde::{ Serialize, Deserialize };
use image::{ ImageFormat, ImageOutputFormat, load_from_memory_with_format as img_loader };
use crate::Model::AllowedImageType;

/*
  Thumbnail Scale Down Factor
  e.g., For a [100x100]px image the thumbnail image will be [25x25]px
*/
const FACTOR: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub uuid: String,
    pub url: String,
    pub metadata: String,
    pub height: usize,
    pub width: usize,
    pub r#type: AllowedImageType
}


/* ADD IMAGE ******************************************************************/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageFrom {Profile, Post, Comment, Chat, VideoThumbnail }
impl std::fmt::Display for ImageFrom {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt,"{:?}", self)
    }
}


pub async fn add(
    uuid: Option<String>,
    data: Vec<u8>,
    from: ImageFrom
) -> Result<ImageInfo, String> {
    /* Validates image format */
    let image_type = match get_image_format(&data) {
        Ok(image_type) => image_type,
        Err(error) => {
            return Err(error);
        }
    };

    /* Extracts image size */
    let img_size = match get_image_size(&data) {
        Ok(size) => size,
        Err(error) => return Err(error)
    };

    let db_conn = sqlite::connect(sqlite::DBF::IMG).unwrap();
    let uuid = match uuid {
        Some(uuid) => uuid,
        None => Uuid::now_v7().to_string()
    };

    let result = db_conn.execute("
        INSERT INTO image (uuid, type, data, height, width, size, created_at, used_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        (
            &uuid,
            &image_type.to_string(),
            &data,
            &img_size.height,
            &img_size.width,
            &data.len(),
            Utc::now().timestamp_millis(),
            &from.to_string()
        )
    );
  

    if let Err(error) = result {
        log::error!("{:?}", error);
        Err(error.to_string())
    } else {
        let image_info = ImageInfo {
            uuid: uuid.clone(),
            url: format!("/image/{uuid}"),
            metadata: format!("/image/metadata/{uuid}"),
            width: img_size.width,
            height: img_size.height,
            r#type: image_type
        };

        Ok(image_info)
    }
}


/* Validates image format based on image blob data */
fn get_image_format(data: &Vec<u8>) -> Result<AllowedImageType, String>{
    if let Some(image_type) = imghdr::from_bytes(data) {
        match image_type {
            imghdr::Type::Gif => Ok(AllowedImageType::Gif),
            imghdr::Type::Png => Ok(AllowedImageType::Png),
            imghdr::Type::Jpeg => Ok(AllowedImageType::Jpeg),
            imghdr::Type::Webp => Ok(AllowedImageType::Webp),
            _ => Err("Unsupported image format!".to_string())
        }
    } else {
        Err("Invalid image format!".to_string())
    }
}

#[derive(Debug, Clone)]
struct ImageSize { width: usize, height: usize }

/* Extracts image dimension from image blob data */
fn get_image_size(data: &Vec<u8>) -> Result<ImageSize, String>{
    match imagesize::blob_size(data) {
        Ok(size) => Ok(ImageSize { width: size.width, height: size.height }),
        Err(error) => {
            log::error!("{:?}", error);
            Err("Invalid image dimensions!".to_string())
        }
    }
}

/*
  Generate Image Thumbnails for fast loading
  NOTE: All thumbnails are generated with `.jpeg` format.
*/
fn _gen_thumbnail(data: &Vec<u8>, dimension: ImageSize, mime: AllowedImageType) -> Result<Vec<u8>, String> {
    let mime_type = match mime {
        AllowedImageType::Gif => ImageFormat::Gif,
        AllowedImageType::Png => ImageFormat::Png,
        AllowedImageType::Jpeg => ImageFormat::Jpeg,
        AllowedImageType::Webp => ImageFormat::WebP,
    };

    let dynamic_image = match img_loader(data, mime_type) {
        Ok(data) => data,
        Err(error) => {
            log::error!("{:?}", error);
            return Err("Invalid image signature!".to_string());
        }
    };

    let image = dynamic_image
        .thumbnail(
            dimension.width as u32 / FACTOR,
            dimension.height as u32 / FACTOR
        )
        .blur(calc_sigma(&dimension));

    let mut image_data: Vec<u8> = Vec::new();
    let result = image.write_to(
        &mut Cursor::new(&mut image_data),
        ImageOutputFormat::Jpeg(50)
    );

    match result {
        Ok(_) => Ok(image_data),
        Err(error) => {
            log::error!("{:?}", error);
            Err("Failed to create thumbnail image!".to_string())
        }
    }
}

/* Calculate the sigma value for the Gaussian Blur effect */
fn calc_sigma(dimension: &ImageSize) -> f32 {
    let pixel_count = dimension.width * dimension.height;

    // Adjust sigma value for different image sizes
    // Just a heuristic, could be improved!
    let sigma = 1.0 + ((pixel_count as f32 / 3000.0) / 1000.0);
    sigma
}

/* END of ADD IMAGE ***********************************************************/

/* REMOVE IMAGE ***************************************************************/

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveFormData { uuid: String }

pub async fn _remove(post_data: RemoveFormData) -> Result<String, String> {
    let db_conn = sqlite::connect(sqlite::DBF::IMG).unwrap();
    let result = db_conn.execute(
        "DELETE FROM image WHERE uuid = ?1",
        params![&post_data.uuid]
    );

    match result {
        Ok(affected_row) => {
            if affected_row == 0 {
                let msg = format!(
                    "UUID: {} - doesn't exists!",
                    &post_data.uuid
                );
                
                Ok(msg)
            } else {
                Err("Successfully removed!".to_string())
            }
        }
        Err(error) => {
          log::error!("{:?}", error);
          Err(error.to_string())
        }
    }
}

/* END of REMOVE IMAGE ********************************************************/