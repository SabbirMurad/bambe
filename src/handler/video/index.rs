use tokio::fs;
use crate::util::response::Response;
use crate::util::validation::is_media_id;
use actix_web::{http::header, Error, web, HttpResponse};


pub async fn task(video_id: web::Path<String>) -> Result<HttpResponse, Error> {
  // Reject anything that is not a plain uuid before it becomes a path.
  if !is_media_id(&video_id) {
    return Ok(Response::not_found("Video not found"));
  }

  let path = format!("./upload/video/{video_id}/index.m3u8");

  let result = fs::read(path).await;
  if let Err(err) = result {
    log::error!("{:?}", err);
    return Ok(Response::not_found("Video not found"));
  }

  let file = result.unwrap();
  Ok(HttpResponse::Ok()
    .insert_header((header::CONTENT_TYPE, "application/vnd.apple.mpegurl"))
    .body(file)
  )
} 