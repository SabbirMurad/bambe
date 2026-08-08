use std::env;
use std::fs;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use actix_web::HttpResponse;
use google_api_auth::AuthenticationHandler;
use reqwest::StatusCode;
use serde_json::{json, Value};
use crate::utils::response::Response;

/// Path to the Firebase service-account JSON (Admin SDK key). Defaults to
/// `firebase_credentials/service_account.json` under the server working dir;
/// override with `FIREBASE_CREDENTIALS_PATH`.
fn credentials_path() -> String {
    env::var("FIREBASE_CREDENTIALS_PATH")
        .unwrap_or_else(|_| "firebase_credentials/service_account.json".to_string())
}

/// Loads and caches the service-account JSON once. Returns `None` when the file
/// is missing/invalid so pushes fail safe (skipped, never a server crash).
fn service_account() -> Option<&'static Value> {
    static SA: OnceLock<Option<Value>> = OnceLock::new();
    SA.get_or_init(|| {
        let path = credentials_path();
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(value) => Some(value),
                Err(error) => {
                    log::error!("firebase service account parse error: {:?}", error);
                    None
                }
            },
            Err(error) => {
                log::warn!(
                    "firebase service account not found at {} ({:?}); pushes disabled",
                    path, error
                );
                None
            }
        }
    })
    .as_ref()
}

/// Public accessor for the Firebase project id (from the service-account key).
/// Used by social-login to pin the expected `aud`/`iss` when verifying ID
/// tokens locally.
pub fn project_id() -> Option<String> {
    firebase_project_id()
}

/// The GCP project id. Read straight from the service-account key (so it always
/// matches the credentials); falls back to `FIREBASE_PROJECT_ID`.
fn firebase_project_id() -> Option<String> {
    if let Some(sa) = service_account() {
        if let Some(pid) = sa.get("project_id").and_then(|v| v.as_str()) {
            return Some(pid.to_string());
        }
    }
    env::var("FIREBASE_PROJECT_ID").ok().filter(|s| !s.is_empty())
}

/// Mints a fresh OAuth2 access token for the FCM HTTP v1 API by signing a JWT
/// with the service-account key and exchanging it at Google's token endpoint.
/// Blocking (crypto + network) — call from `spawn_blocking`.
fn mint_access_token() -> Option<String> {
    let sa = service_account()?;
    let json_string = sa.to_string();

    let handler = AuthenticationHandler::new(json_string.into());
    let token = handler.get_access_token_model(vec![
        "https://www.googleapis.com/auth/firebase.messaging".into(),
    ]);

    if token.access_token.is_empty() {
        log::error!("firebase token endpoint returned an empty access token");
        None
    } else {
        Some(token.access_token)
    }
}

/// Cached OAuth token. FCM tokens live ~1h; we cache for 50 min and re-mint on
/// demand. Still blocking — always resolved via `spawn_blocking`.
fn firebase_access_token() -> Option<String> {
    static CACHE: OnceLock<Mutex<Option<(String, Instant)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));

    if let Ok(guard) = cache.lock() {
        if let Some((token, expires_at)) = guard.as_ref() {
            if *expires_at > Instant::now() {
                return Some(token.clone());
            }
        }
    }

    let token = mint_access_token()?;
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((token.clone(), Instant::now() + Duration::from_secs(3000)));
    }
    Some(token)
}

/// Sends a single FCM HTTP v1 message to one device token.
///
/// Pass `data` for a data-only message (the app's `firebase_api.dart`
/// `handle_message` reads the `data` keys directly, giving consistent handling
/// in foreground and background). Optionally include a `notification` block via
/// `title`/`body`/`image`.
///
/// Fails safe: if Firebase isn't configured (no credentials) it logs and returns
/// `Ok(())` so callers never treat "unconfigured" as an error.
pub async fn send_notification(
    fcm_token: &str,
    title: Option<String>,
    body: Option<String>,
    image: Option<String>,
    data: Option<Value>,
) -> Result<(), HttpResponse> {
    let project_id = match firebase_project_id() {
        Some(id) => id,
        None => {
            log::warn!("FCM not configured (no project id) — skipping push");
            return Ok(());
        }
    };

    let access_token = match tokio::task::spawn_blocking(firebase_access_token).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            log::warn!("FCM not configured (no access token) — skipping push");
            return Ok(());
        }
        Err(error) => {
            log::error!("firebase token task join error: {:?}", error);
            return Err(Response::internal_server_error(&error.to_string()));
        }
    };

    // Build the `message` object. All FCM v1 `data` values must be strings.
    let mut message = json!({
        "token": fcm_token,
        "android": { "priority": "high" },
    });

    if title.is_some() || body.is_some() || image.is_some() {
        message["notification"] = json!({
            "title": title,
            "body": body,
            "image": image,
        });
    }

    if let Some(data) = data {
        message["data"] = data;
        // Wake the iOS app for data-only messages so it can build the local
        // notification itself.
        message["apns"] = json!({
            "payload": { "aps": { "content-available": 1 } }
        });
    }

    let url = format!(
        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
        project_id
    );

    let result = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token)
        .json(&json!({ "message": message }))
        .send()
        .await;

    let response = match result {
        Ok(response) => response,
        Err(error) => {
            log::error!("FCM send failed: {:?}", error);
            return Err(Response::internal_server_error(&error.to_string()));
        }
    };

    if response.status() == StatusCode::OK {
        Ok(())
    } else {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        log::error!("FCM send returned {}: {}", status, text);
        Err(Response::internal_server_error(&text))
    }
}
