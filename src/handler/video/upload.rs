use std::env;
use std::process::Stdio;
use std::sync::OnceLock;
use uuid::Uuid;
use futures::TryStreamExt;
use actix_multipart::Multipart;
use crate::util::response::Response;
use tokio::fs::{create_dir_all, remove_dir_all, remove_file, File};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout, Duration};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use actix_web::{Error, HttpResponse, HttpRequest, http::header::CONTENT_LENGTH};

/// Hard cap on an uploaded video (50 MB), enforced against the bytes actually
/// received rather than the declared length.
const MAX_FILE_SIZE: usize = 52_428_800;

const UPLOAD_DIR: &str = "./upload/video/";

/// Reads a positive integer from the environment, falling back to `default`.
fn positive_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default)
}

/// Wall-clock limit for a single transcode. Without one, a crafted or
/// pathologically complex input keeps ffmpeg busy indefinitely.
fn ffmpeg_timeout() -> Duration {
    static VALUE: OnceLock<Duration> = OnceLock::new();
    *VALUE.get_or_init(|| Duration::from_secs(positive_env("FFMPEG_TIMEOUT_SECONDS", 120)))
}

/// How long a request waits for a free transcode slot before giving up. Bounded
/// on purpose: an unbounded queue is itself the resource-exhaustion problem,
/// just moved from ffmpeg processes to parked requests holding uploaded files.
fn queue_wait() -> Duration {
    static VALUE: OnceLock<Duration> = OnceLock::new();
    *VALUE.get_or_init(|| Duration::from_secs(positive_env("TRANSCODE_QUEUE_WAIT_SECONDS", 30)))
}

/// Caps how many transcodes run at once.
///
/// Process-wide, not per-worker: a `OnceLock` static is shared across every
/// actix worker thread, so the limit is the real number of ffmpeg processes.
/// Building it inside the app factory would give each worker its own, silently
/// multiplying the limit by the worker count — the same trap the rate-limiter
/// comment in `main.rs` warns about.
fn transcode_slots() -> &'static Semaphore {
    static VALUE: OnceLock<Semaphore> = OnceLock::new();
    VALUE.get_or_init(|| {
        let slots = positive_env("MAX_CONCURRENT_TRANSCODES", 2) as usize;
        log::info!("video: at most {} concurrent transcode(s)", slots);
        Semaphore::new(slots)
    })
}

/// Attempts before giving up on a removal, and the pause between them.
///
/// Killing a process does not synchronously release the file handles it held —
/// on Windows a removal issued immediately after `kill_on_drop` fires loses the
/// race and fails with "The process cannot access the file because it is being
/// used by another process" (os error 32). Retrying briefly closes that window.
/// This matters: without it every timed-out transcode leaks its source file, so
/// the fix for a CPU-exhaustion vector would hand back a disk-exhaustion one.
const REMOVE_ATTEMPTS: u32 = 10;
const REMOVE_RETRY_DELAY: Duration = Duration::from_millis(50);

/// Best-effort cleanup. The caller is already on a failure path (or done with
/// the source file), so a removal error is logged rather than surfaced.
/// A missing path is not an error — several failure paths run before it exists.
async fn remove_quietly(path: &str) {
    for attempt in 1..=REMOVE_ATTEMPTS {
        match remove_file(path).await {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                if attempt == REMOVE_ATTEMPTS {
                    log::warn!(
                        "could not remove {} after {} attempts: {:?}",
                        path, REMOVE_ATTEMPTS, error
                    );
                    return;
                }
                sleep(REMOVE_RETRY_DELAY).await;
            }
        }
    }
}

/// As `remove_quietly`, for the segment output directory.
async fn remove_dir_quietly(path: &str) {
    for attempt in 1..=REMOVE_ATTEMPTS {
        match remove_dir_all(path).await {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                if attempt == REMOVE_ATTEMPTS {
                    log::warn!(
                        "could not remove {} after {} attempts: {:?}",
                        path, REMOVE_ATTEMPTS, error
                    );
                    return;
                }
                sleep(REMOVE_RETRY_DELAY).await;
            }
        }
    }
}

pub async fn task(mut payload: Multipart, req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(
        &req,
        AccessRequirement::AnyToken
    )?;

    let _user_id = user.user_id;

    /*
      Cheap pre-check so an obviously oversized upload is rejected before a byte
      is read. This is an optimisation ONLY — it is not the limit. A chunked
      request carries no Content-Length at all, so a client that simply omits the
      header skips this branch entirely. The real cap is enforced in the write
      loop below, against bytes actually received.
    */
    let declared_length: usize = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    if declared_length > MAX_FILE_SIZE {
        return Ok(Response::payload_too_large("File size too large"));
    }

    //check if folder exists
    let result = create_dir_all(UPLOAD_DIR).await;
    if let Err(error) = result {
        return Ok(Response::internal_server_error(&format!("Could not prepare upload directory: {:?}", error)));
    }

    let result = payload.try_next().await;
    if let Err(error) = result {
        return Ok(Response::internal_server_error(&format!("Could not read upload: {:?}", error)));
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

    let video_path = format!("{UPLOAD_DIR}file-{uuid}");
    let segment_dir = format!("{UPLOAD_DIR}{uuid}");

    let mut saved_file = match File::create(&video_path).await {
        Ok(file) => file,
        Err(error) => {
            return Ok(Response::internal_server_error(&format!("Could not open upload file: {:?}", error)));
        }
    };

    /*
      Stream the body to disk, stopping the moment the cap is crossed so a
      client cannot fill the disk by withholding Content-Length.
    */
    let mut written: usize = 0;
    loop {
        let chunk = match field.try_next().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => {
                // The previous `while let Ok(Some(chunk))` treated a stream error
                // as a clean end-of-body, so a truncated upload was transcoded
                // and returned 200 as if it had arrived intact.
                log::error!("{:?}", error);
                drop(saved_file);
                remove_quietly(&video_path).await;
                return Ok(Response::bad_request("Upload stream failed"));
            }
        };

        written += chunk.len();
        if written > MAX_FILE_SIZE {
            drop(saved_file);
            remove_quietly(&video_path).await;
            return Ok(Response::payload_too_large("File size too large"));
        }

        if let Err(error) = saved_file.write_all(&chunk).await {
            drop(saved_file);
            remove_quietly(&video_path).await;
            return Ok(Response::internal_server_error(&format!("Could not write upload: {:?}", error)));
        }
    }

    if let Err(error) = saved_file.flush().await {
        drop(saved_file);
        remove_quietly(&video_path).await;
        return Ok(Response::internal_server_error(&format!("Could not write upload: {:?}", error)));
    }

    // Close the handle before ffmpeg reads the file and before we delete it —
    // on Windows an open handle makes the later removal fail.
    drop(saved_file);

    if written == 0 {
        remove_quietly(&video_path).await;
        return Ok(Response::bad_request("Uploaded file is empty"));
    }

    let result = create_dir_all(&segment_dir).await;
    if let Err(error) = result {
        remove_quietly(&video_path).await;
        return Ok(Response::internal_server_error(&format!("Could not prepare output directory: {:?}", error)));
    }

    let hls_path = format!("{segment_dir}/index.m3u8");
    let segment_pattern = format!("{segment_dir}/segment-%04d.ts");

    /*
      Take a transcode slot before spawning ffmpeg — not before reading the body,
      which is network-bound and would hold capacity idle for the whole upload.
    */
    let permit = match timeout(queue_wait(), transcode_slots().acquire()).await {
        Ok(Ok(permit)) => permit,
        Ok(Err(error)) => {
            remove_quietly(&video_path).await;
            remove_dir_quietly(&segment_dir).await;
            return Ok(Response::internal_server_error(&format!("Video processing unavailable (transcode semaphore closed): {:?}", error)));
        }
        Err(_elapsed) => {
            log::warn!("no transcode slot within {:?}; refusing upload", queue_wait());
            remove_quietly(&video_path).await;
            remove_dir_quietly(&segment_dir).await;
            return Ok(Response::service_unavailable(
                "Server is busy transcoding; please retry shortly",
            ));
        }
    };

    /*
      tokio::process::Command, not std::process::Command.

      The std version blocks the calling thread until ffmpeg exits. Inside an
      async handler that is a tokio worker thread, so a single multi-second
      transcode stalled every other request scheduled on that worker.

      `kill_on_drop(true)` is what makes the timeout below actually work.
      `wait_with_output()` takes the child by value, so once that future is
      cancelled there is no handle left to call `.kill()` on — dropping the child
      is the only way to reap it, and this flag makes that drop kill the process
      rather than orphan a runaway ffmpeg.

      stdout/stderr must be piped explicitly: `output()` does that for you, but
      `spawn()` + `wait_with_output()` inherits by default and captures nothing.
    */
    let child = Command::new("ffmpeg")
        .args([
            "-i", &video_path,
            "-codec:v", "libx264",
            "-codec:a", "aac",
            "-hls_time", "4",
            "-hls_playlist_type", "vod",
            "-hls_segment_filename", &segment_pattern,
            "-start_number", "0", &hls_path
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();

    let child = match child {
        Ok(child) => child,
        Err(error) => {
            drop(permit);
            remove_quietly(&video_path).await;
            remove_dir_quietly(&segment_dir).await;
            return Ok(Response::internal_server_error(&format!("Video processing unavailable (could not run ffmpeg): {:?}", error)));
        }
    };

    let result = timeout(ffmpeg_timeout(), child.wait_with_output()).await;

    // Release the slot as soon as ffmpeg is done, before the cleanup I/O below.
    drop(permit);

    let output = match result {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            remove_quietly(&video_path).await;
            remove_dir_quietly(&segment_dir).await;
            return Ok(Response::internal_server_error(&format!("Video processing unavailable (ffmpeg failed): {:?}", error)));
        }
        Err(_elapsed) => {
            // The child was moved into `wait_with_output`, so cancelling that
            // future dropped it — and `kill_on_drop(true)` killed the process.
            log::error!("ffmpeg exceeded {:?}; killed", ffmpeg_timeout());
            remove_quietly(&video_path).await;
            remove_dir_quietly(&segment_dir).await;
            // 400 rather than 5xx: the actionable fix is a shorter or simpler
            // input, and the request is not retryable as-is.
            return Ok(Response::bad_request(
                "Video took too long to process; try a shorter or smaller file",
            ));
        }
    };

    /*
      The old code checked only whether the process *started*. ffmpeg reports a
      corrupt or unsupported input through its exit status, so a bad upload
      returned 200 with an id whose segment directory was empty — the failure
      only surfaced later, at playback.
    */
    if !output.status.success() {
        log::error!(
            "ffmpeg exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        remove_quietly(&video_path).await;
        remove_dir_quietly(&segment_dir).await;
        return Ok(Response::bad_request("Could not process this video file"));
    }

    remove_quietly(&video_path).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(uuid))
}
