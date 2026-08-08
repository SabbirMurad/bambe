use std::io::Read;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use actix_web::{http::header, HttpRequest, HttpResponse, web, Responder};

const CHUNK_SIZE: u64 = 1024 * 1024;

pub async fn task(req: HttpRequest, video_id: web::Path<String>) -> impl Responder {
    let file_path = format!("./uploaded_video/{video_id}");
    let mut file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    let file_size = match file.metadata() {
        Ok(meta) => meta.len(),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let range_header = req.headers().get(header::RANGE);
    let range = if let Some(range_header) = range_header {
        let range_str = range_header.to_str().unwrap();

        let parts: Vec<&str> = range_str
            .trim_start_matches("bytes=")
            .split('-')
            .collect();

        let start = parts[0].parse::<u64>().unwrap_or(0);

        let end = parts
            .get(1)
            .and_then(|&e| e.parse::<u64>().ok())
            .unwrap_or(file_size - 1);

        (start, end)
    } else {
        (0, CHUNK_SIZE.min(file_size - 1))
    };

    let (start, end) = range;

    if file.seek(SeekFrom::Start(start)).is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    let mut buffer = vec![0; (end - start + 1) as usize];
    if file.read_exact(&mut buffer).is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::PartialContent()
        .insert_header((
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, file_size),
        ))
        .insert_header((header::CONTENT_TYPE, "video/mp4"))
        .insert_header((header::ACCEPT_RANGES, "bytes"))
        .body(buffer)
}