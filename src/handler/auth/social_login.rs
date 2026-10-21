use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use uuid::Uuid;
use chrono::Utc;
use serde_json::json;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_session::Session;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use crate::BuiltIns::{mongo::MongoDB, jwt};
use crate::util::response::Response;
use actix_web::{web, Error, HttpResponse};
use crate::model::Account;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    /// "google" | "apple" — informational; verification is provider-agnostic
    /// because both arrive as Firebase ID tokens.
    #[allow(dead_code)]
    provider: String,
    /// The Firebase ID token (a JWT signed by Google).
    token: String,
}

/// Claims we read from a verified Firebase ID token. `aud`/`iss`/`exp` are
/// checked by `Validation` (below), not read here.
#[derive(Debug, Deserialize)]
struct FirebaseClaims {
    /// The stable Firebase user id.
    sub: String,
    email: Option<String>,
    name: Option<String>,
}

/// One RSA public key from Google's securetoken JWK set.
#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

/// Google's public keys for Firebase ID tokens, as JWKs (RSA `n`/`e`
/// components — no PEM/cert parsing needed).
const GOOGLE_JWK_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

/// Fetches (and caches ~1h) Google's public signing keys keyed by `kid`.
/// Firebase rotates these infrequently, so after the first call this is
/// effectively free — no per-login network round-trip to Google.
async fn google_signing_keys() -> Option<HashMap<String, Jwk>> {
    static CACHE: OnceLock<Mutex<Option<(HashMap<String, Jwk>, Instant)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));

    if let Ok(guard) = cache.lock() {
        if let Some((keys, expires_at)) = guard.as_ref() {
            if *expires_at > Instant::now() {
                return Some(keys.clone());
            }
        }
    }

    let response = match reqwest::Client::new().get(GOOGLE_JWK_URL).send().await {
        Ok(response) => response,
        Err(error) => {
            log::error!("fetch google signing keys: {:?}", error);
            return None;
        }
    };
    let set: JwkSet = match response.json().await {
        Ok(set) => set,
        Err(error) => {
            log::error!("parse google signing keys: {:?}", error);
            return None;
        }
    };

    let map: HashMap<String, Jwk> =
        set.keys.into_iter().map(|k| (k.kid.clone(), k)).collect();

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((map.clone(), Instant::now() + Duration::from_secs(3600)));
    }
    Some(map)
}

/// Verifies a Firebase ID token entirely locally: RS256 signature against
/// Google's published keys, plus `aud` == project id, `iss` ==
/// `securetoken.google.com/<project id>`, and expiry. No API key, no user
/// lookup call. Returns the token claims on success.
async fn verify_firebase_token(token: &str, project_id: &str) -> Option<FirebaseClaims> {
    let header = decode_header(token).ok()?;
    let kid = header.kid?;

    let keys = google_signing_keys().await?;
    let jwk = keys.get(&kid)?;
    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).ok()?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[project_id]);
    validation.set_issuer(&[format!("https://securetoken.google.com/{}", project_id)]);

    match decode::<FirebaseClaims>(token, &key, &validation) {
        Ok(data) => Some(data.claims),
        Err(error) => {
            log::warn!("firebase token verification failed: {:?}", error);
            None
        }
    }
}

pub async fn task(
    body: web::Json<ReqBody>,
    session: Session,
) -> Result<HttpResponse, Error> {
    // The expected `aud`/`iss` come from the same service-account key the push
    // sender uses, so the two can never drift apart.
    let project_id = match crate::Integrations::Firebase::project_id() {
        Some(id) => id,
        None => {
            return Ok(Response::internal_server_error(
                "Firebase credentials not configured",
            ));
        }
    };

    let claims = match verify_firebase_token(&body.token, &project_id).await {
        Some(claims) => claims,
        None => return Ok(Response::unauthorized("Invalid or expired token")),
    };

    let _firebase_uid = claims.sub; // kept for potential provider-uid linking

    let email = match claims.email {
        Some(email) => email.to_lowercase(),
        None => return Ok(Response::bad_request("No email associated with this account")),
    };

    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();
    let account_col = db.collection::<Account::AccountCore>("account_core");

    // Find existing account by email or create one.
    let existing = account_col.find_one(doc!{ "email_address": &email }).await;
    if let Err(error) = existing {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let user_id = match existing.unwrap() {
        Some(account) => account.uuid,
        None => {
            // Create a new account for first-time social login.
            let new_id = Uuid::now_v7().to_string();
            let display_name = claims.name.unwrap_or_default();
            let parts: Vec<&str> = display_name.splitn(2, ' ').collect();
            let first_name = parts.first().copied().unwrap_or("User").to_string();
            let last_name = parts.get(1).copied().unwrap_or("").to_string();

            let new_account = Account::AccountCore {
                uuid: new_id.clone(),
                email_address: email.clone(),
                username: format!("user_{}", &new_id[..8]),
                password: String::new(),
                email_verified: true,
                role: Account::AccountRole::User,
                // New social users go through the same selfie + setup +
                // admin-approval onboarding as everyone else.
                two_a_factor_auth_enabled: false,
                two_a_factor_auth_updated: None,
                created_at: now,
                suspended_at: None,
                suspended_by: None,
            };

            if let Err(error) = account_col.insert_one(&new_account).await {
                log::error!("{:?}", error);
                return Ok(Response::internal_server_error(&error.to_string()));
            }

            // Create basic account profile.
            let profile = Account::AccountProfile {
                uuid: new_id.clone(),
                first_name,
                last_name,
                phone_number: None,
                profile_picture: None,
                modified_at: now,
            };
            let _ = db.collection::<Account::AccountProfile>("account_profile")
                .insert_one(&profile).await;

            new_id
        }
    };

    let (access_token, valid_minutes) = match jwt::access_token::generate_default(&user_id, Account::AccountRole::User,) {
        Ok(pair) => pair,
        Err(error) => return Ok(Response::internal_server_error(&format!("could not mint an access token: {:?}", error))),
    };
    let access_token_valid_till = now + (valid_minutes as i64 * 60 * 1000);

    let refresh_token = match jwt::refresh_token::new(&user_id) {
        Ok(token) => token,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    };

    session.insert("user_id", &user_id).ok();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "access_token": access_token,
        "access_token_valid_till": access_token_valid_till,
        "refresh_token": refresh_token,
        "user_id": user_id,
        "role": "User",
    })))
}
