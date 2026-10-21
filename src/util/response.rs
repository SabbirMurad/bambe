use actix_web::error::{InternalError, JsonPayloadError};
use actix_web::{Error, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub message: String,
}

/// Render `web::Json` extractor failures (missing/invalid fields, bad body) in
/// the app's `{ "message": ... }` shape instead of actix's default plain text.
/// Wire it globally via `web::JsonConfig::default().error_handler(json_error_handler)`.
pub fn json_error_handler(err: JsonPayloadError, _req: &HttpRequest) -> Error {
    // serde's error text names the expected fields / types and leaks the body
    // structure — log it server-side, but return a generic message to the client.
    log::warn!("Rejected malformed JSON body: {}", err);
    let response = Response::bad_request("Invalid or malformed request body.");
    InternalError::from_response(err, response).into()
}

impl Response {
    pub fn conflict(message: &str) -> HttpResponse {
        HttpResponse::Conflict()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn bad_request(message: &str) -> HttpResponse {
        HttpResponse::BadRequest()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn unauthorized(message: &str) -> HttpResponse {
        HttpResponse::Unauthorized()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn _payment_required(message: &str) -> HttpResponse {
        HttpResponse::PaymentRequired()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn forbidden(message: &str) -> HttpResponse {
        HttpResponse::Forbidden()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn not_found(message: &str) -> HttpResponse {
        HttpResponse::NotFound()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    /// 413 — the request body exceeded a size limit the server enforces.
    pub fn payload_too_large(message: &str) -> HttpResponse {
        HttpResponse::PayloadTooLarge()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn _gone(message: &str) -> HttpResponse {
        HttpResponse::Gone()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn ok_message(message: &str) -> HttpResponse {
        HttpResponse::Ok()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    pub fn _not_implemented(message: &str) -> HttpResponse {
        HttpResponse::NotImplemented()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    /// 502 — an upstream dependency we don't control (e.g. the email relay)
    /// failed. Unlike a 500, the message is passed through so the caller learns
    /// the actionable cause (e.g. "the invite couldn't be emailed").
    /// 503 — the server is temporarily at capacity. Distinct from 500: the
    /// request was well-formed and retrying later is expected to work.
    pub fn service_unavailable(message: &str) -> HttpResponse {
        HttpResponse::ServiceUnavailable()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    /// Completes the 5xx set. Nothing proxies upstream yet, but a template
    /// should not make you hand-roll a response the first time you do.
    #[allow(dead_code)]
    pub fn bad_gateway(message: &str) -> HttpResponse {
        HttpResponse::BadGateway()
            .content_type("application/json")
            .json(Response {
                message: message.to_string(),
            })
    }

    /// 500 response. The real `detail` is never sent to the client — it's logged
    /// against a short reference id, and the client gets a generic message carrying
    /// that same id. A user can quote the ref so we can grep the logs for the cause.
    pub fn internal_server_error(detail: &str) -> HttpResponse {
        /*
          The reference IS the request id, so the id in the access-log line, the
          id in this error line and the id the client is shown are all the same
          value. Quoting it now reconstructs the whole request - method, path,
          status, latency - not just the one line that reported the failure.

          Falls back to a fresh id outside a request (a cron sweep, a spawned
          task), where there is no request to correlate with.
        */
        let reference = crate::Middleware::RequestId::current()
            .unwrap_or_else(crate::Middleware::RequestId::generate);

        log::error!("[{}] {}", reference, detail);

        HttpResponse::InternalServerError()
            .content_type("application/json")
            .json(json!({
                "message": format!(
                    "An unexpected error occurred. Please try again. (ref: {})", reference
                ),
                "reference": reference,
            }))
    }
}
