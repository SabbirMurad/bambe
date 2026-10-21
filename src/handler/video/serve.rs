use std::io::SeekFrom;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use actix_web::{http::header, HttpRequest, HttpResponse, web, Responder};
use crate::util::validation::is_media_id;

/// Largest slice returned in a single response, and therefore the largest
/// buffer this handler will ever allocate.
const CHUNK_SIZE: u64 = 1024 * 1024;

/// Parses a single byte-range request per RFC 9110 §14.1.
///
/// Returns `None` for anything malformed or unsatisfiable, which the caller
/// turns into 416. Every arithmetic step here is on client-controlled input, so
/// nothing subtracts without knowing the operands.
///
/// The previous implementation got three things wrong:
///   - `range_str.to_str().unwrap()` panicked on a non-UTF-8 header;
///   - `end` was whatever the client asked for, and the response buffer was
///     allocated from it, so `Range: bytes=0-99999999999` attempted a 100 GB
///     allocation;
///   - an inverted range (`end < start`) underflowed `end - start + 1` into a
///     near-`usize::MAX` allocation.
fn parse_range(raw: &str, file_size: u64) -> Option<(u64, u64)> {
    // Only `bytes` ranges, and only one of them: multipart/byteranges responses
    // are not implemented, so a multi-range request is refused rather than
    // silently answered with just the first range.
    let spec = raw.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }

    let (start_text, end_text) = spec.split_once('-')?;
    let start_text = start_text.trim();
    let end_text = end_text.trim();

    let (start, end) = if start_text.is_empty() {
        // Suffix range: `bytes=-500` means the LAST 500 bytes. The old code read
        // it as start=0, end=500 and returned the wrong part of the file.
        let wanted: u64 = end_text.parse().ok()?;
        if wanted == 0 {
            return None;
        }
        (file_size.saturating_sub(wanted), file_size - 1)
    } else {
        let start: u64 = start_text.parse().ok()?;
        let end = if end_text.is_empty() {
            // Open range: `bytes=500-` means to the end of the file.
            file_size - 1
        } else {
            end_text.parse::<u64>().ok()?.min(file_size - 1)
        };
        (start, end)
    };

    if start > end || start >= file_size {
        return None;
    }

    Some((start, end))
}

pub async fn task(req: HttpRequest, video_id: web::Path<String>) -> impl Responder {
    // Validate BEFORE the value reaches a path. See util::validation::is_media_id
    // for why this is a whitelist rather than a filter. 404 rather than 400: a
    // rejected id should look exactly like one that does not exist.
    if !is_media_id(&video_id) {
        return HttpResponse::NotFound().finish();
    }

    let file_path = format!("./uploaded_video/{video_id}");

    let mut file = match File::open(file_path).await {
        Ok(file) => file,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    let file_size = match file.metadata().await {
        Ok(meta) => meta.len(),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // Guard before any `file_size - 1`. A zero-byte file means a failed or
    // partial upload; there is no range of it that can be satisfied.
    if file_size == 0 {
        return HttpResponse::NotFound().finish();
    }

    let (start, end) = match req.headers().get(header::RANGE) {
        Some(value) => {
            let raw = match value.to_str() {
                Ok(raw) => raw,
                // Non-UTF-8 header. Previously `.unwrap()`, i.e. a panic.
                Err(_) => {
                    return HttpResponse::RangeNotSatisfiable()
                        .insert_header((header::CONTENT_RANGE, format!("bytes */{}", file_size)))
                        .finish();
                }
            };

            match parse_range(raw, file_size) {
                Some(range) => range,
                None => {
                    return HttpResponse::RangeNotSatisfiable()
                        .insert_header((header::CONTENT_RANGE, format!("bytes */{}", file_size)))
                        .finish();
                }
            }
        }
        None => (0, (CHUNK_SIZE - 1).min(file_size - 1)),
    };

    // Hard cap independent of what was requested. `parse_range` already bounds
    // `end` to the file, but this is what guarantees the allocation below can
    // never exceed CHUNK_SIZE no matter how the range was expressed.
    let end = end.min(start + CHUNK_SIZE - 1).min(file_size - 1);

    // Safe: `parse_range` guarantees start <= end, and the clamp above only
    // lowers `end` to values >= start.
    let length = end - start + 1;

    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    let mut buffer = vec![0u8; length as usize];
    if file.read_exact(&mut buffer).await.is_err() {
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
