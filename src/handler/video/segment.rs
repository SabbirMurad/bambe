use tokio::fs;
use serde::{Deserialize, Serialize};
use crate::util::response::Response;
use crate::util::validation::{is_hls_file_name, is_media_id};
use actix_web::{http::header, Error, web, HttpResponse};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PathParams { video_id: String, segment_name: String }

pub async fn task(params: web::Path<PathParams>) -> Result<HttpResponse, Error> {
    let PathParams { video_id, segment_name } = params.into_inner();

    /*
      BOTH halves are attacker-controlled and both were interpolated raw. The
      segment name is the more dangerous of the two, because it sits at the end
      of the path: `%2e%2e%2f%2e%2e%2fCargo.toml` walked straight out of the
      upload directory and returned the file.
    */
    if !is_media_id(&video_id) || !is_hls_file_name(&segment_name) {
        return Ok(Response::not_found("File not found"));
    }

    let file_path = format!("./upload/video/{video_id}/{segment_name}");

    let result = fs::read(file_path).await;

    if let Err(_) = result {
        return Ok(Response::not_found("File not found"));
    }

    let file = result.unwrap();

    let content_type = if segment_name.ends_with(".m3u8") {
        "application/x-mpegURL"
    } else if segment_name.ends_with(".ts") {
        "video/mp2t"
    } else {
        "application/octet-stream"
    };

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, content_type))
        .body(file))
}