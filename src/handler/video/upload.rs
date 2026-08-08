use uuid::Uuid;
use futures::TryStreamExt;
use actix_multipart::Multipart;
use std::{fs, process::Command};
use crate::utils::response::Response;
use tokio::{fs::{create_dir_all, File}, io::AsyncWriteExt};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use actix_web::{Error, HttpResponse, HttpRequest, http::header::CONTENT_LENGTH};


pub async fn task(mut payload: Multipart, req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(
        &req,
        AccessRequirement::AnyToken
    )?;

    let _user_id = user.user_id;

    let content_length: usize = match req.headers().get(CONTENT_LENGTH) {
        Some(header_value) => header_value
            .to_str().unwrap_or("0").parse().unwrap(),
        None => "0".parse().unwrap(),
    };

    //50 MB
    let max_file_size: usize = 52_428_800;
    let dir: &str = "./upload/video/";

    //check if folder exists
    let result = create_dir_all(dir).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    if content_length > max_file_size {
        return Ok(Response::bad_request("File size too large"));
    }

    let result = payload.try_next().await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::bad_request("Field not found"));
    }

    let mut field = option.unwrap();
    let field_name = field.name();
    if field_name.is_none() {
        return Ok(Response::bad_request("Field name not found"));
    }

    let field_name = field_name.unwrap();
    if field_name != "video" {
        return Ok(Response::forbidden(
            &format!("Field {} is not allowed", field_name)
        ));
    }

    let uuid = Uuid::new_v4().to_string();

    let video_path = format!("{dir}file-{uuid}");
    let mut segment_path = format!("{dir}{uuid}");
    let mut saved_file: File = File::create(&video_path).await.unwrap();
    while let Ok(Some(chunk)) = field.try_next().await {
        let _ = saved_file.write_all(&chunk).await.unwrap();
    }
  
    let result = fs::create_dir_all(&segment_path);
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let hls_path = format!("{segment_path}/index.m3u8");
    segment_path = format!("{segment_path}/segment-%04d.ts");

    let result = Command::new("ffmpeg")
        .args([
            "-i", &video_path,
            "-codec:v", "libx264",
            "-codec:a", "aac",
            "-hls_time", "4",
            "-hls_playlist_type", "vod",
            "-hls_segment_filename", &segment_path,
            "-start_number", "0", &hls_path
        ])
        .output();

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let _ = fs::remove_file(video_path);

    Ok(HttpResponse::Ok().content_type("application/json").json(uuid))
}