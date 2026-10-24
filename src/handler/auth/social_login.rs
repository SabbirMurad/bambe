use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use uuid::Uuid;
use chrono::Utc;
use serde_json::json;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_session::Session;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use crate::BuiltIns::{mongo::MongoDB, jwt};
use crate::util::response::Response;
use actix_web::{web, Error, HttpResponse};
use crate::model::Account;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    /// "google" | "facebook". Only a fallback label: the token itself says
    /// which provider signed the person in, and that is what is recorded.
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
    /// Whether the provider vouches for the address. Google always does;
    /// Facebook often does not, which is why an email alone is never enough
    /// to open an existing account — see `task`.
    #[serde(default)]
    email_verified: bool,
    name: Option<String>,
    /// The provider's profile photo, as a URL on its own CDN.
    picture: Option<String>,
    firebase: Option<FirebaseInfo>,
}

#[derive(Debug, Deserialize)]
struct FirebaseInfo {
    /// "google.com", "facebook.com", ...
    sign_in_provider: String,
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

    let provider = claims
        .firebase
        .as_ref()
        .map(|f| f.sign_in_provider.clone())
        .unwrap_or_else(|| body.provider.clone());

    let email = match claims.email.as_deref().map(str::trim) {
        Some(email) if !email.is_empty() => email.to_lowercase(),
        _ => return Ok(Response::bad_request(
            "Your account didn't share an email address with us. Allow email access when asked, or sign up with your email instead.",
        )),
    };

    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();
    let account_col = db.collection::<Account::AccountCore>("account_core");
    let identities = db.collection::<Document>("social_identity");
    // Scoped to shoppers, as sign_in.rs is: this mints a User token, so it
    // must never resolve to a row of any other role.
    let user_role = Account::AccountRole::User.to_string();

    /*
      Which account this sign-in opens, in order of trust:

        1. The identity itself. Once a Google or Facebook identity (the
           Firebase uid) has signed in, it is linked to its account and
           matched by that from then on — whatever its email says later.
        2. The email, only when the provider vouches for it. Facebook often
           hands over an address it has not verified; matching on that would
           let anyone open an account by putting its owner's email on a
           Facebook profile.
        3. Otherwise a new account.
    */
    let mut account: Option<Account::AccountCore> = None;
    let linked = identities
        .find_one(doc!{ "firebase_uid": &claims.sub })
        .await
        .ok()
        .flatten()
        .and_then(|d| d.get_str("user_id").ok().map(str::to_string));
    if let Some(user_id) = linked {
        account = account_col
            .find_one(doc!{ "uuid": &user_id, "role": &user_role })
            .await
            .ok()
            .flatten();
        if account.is_none() {
            // The account was deleted; let this sign-in start afresh.
            let _ = identities.delete_one(doc!{ "firebase_uid": &claims.sub }).await;
        }
    }
    let first_time = account.is_none();

    if first_time {
        let by_email = match account_col.find_one(doc!{ "email_address": &email, "role": &user_role }).await {
            Ok(found) => found,
            Err(error) => {
                log::error!("{:?}", error);
                return Ok(Response::internal_server_error(&error.to_string()));
            }
        };

        account = match by_email {
            Some(_) if !claims.email_verified => {
                return Ok(Response::forbidden(
                    "An account with this email already exists. Sign in with your email and password, or with Google, instead.",
                ));
            }
            // An address nobody has proven yet: someone signed up with it
            // and never entered the code. Handing that account to whoever
            // now owns the email would hand them a password a stranger chose.
            Some(existing) if !existing.email_verified => {
                return Ok(Response::forbidden(
                    "An account with this email is still waiting to be verified. Use the code we emailed, or reset its password, then try again.",
                ));
            }
            Some(existing) => Some(existing),
            None => {
                let new_id = Uuid::now_v7().to_string();
                // The provider's name fills the profile once, here. After
                // that it is the customer's to edit, and no later sign-in
                // overwrites it.
                let display_name = claims.name.clone().unwrap_or_default();
                let parts: Vec<&str> = display_name.trim().splitn(2, ' ').collect();
                let first_name = parts.first().copied().filter(|s| !s.is_empty()).unwrap_or("User").to_string();
                let last_name = parts.get(1).copied().unwrap_or("").to_string();

                let new_account = Account::AccountCore {
                    uuid: new_id.clone(),
                    email_address: email.clone(),
                    username: format!("user_{}", &new_id[..8]),
                    password: String::new(),
                    email_verified: claims.email_verified,
                    role: Account::AccountRole::User,
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

                let profile = Account::AccountProfile {
                    uuid: new_id.clone(),
                    first_name,
                    last_name,
                    phone_number: None,
                    profile_picture: None,
                    social_picture: None,
                    modified_at: now,
                };
                let _ = db.collection::<Account::AccountProfile>("account_profile")
                    .insert_one(&profile).await;

                Some(new_account)
            }
        };
    }

    let account = account.expect("resolved above");

    // Same rule as the password sign-in: a suspension holds whichever way
    // the customer tries to get back in.
    if account.suspended_at.is_some() {
        return Ok(Response::forbidden("This account has been suspended"));
    }
    let user_id = account.uuid.clone();

    if first_time {
        // A duplicate here is a concurrent first sign-in that got there
        // first, which links the same pair — nothing to do.
        let _ = identities.insert_one(doc!{
            "firebase_uid": &claims.sub,
            "user_id": &user_id,
            "provider": &provider,
            "email": &email,
            "created_at": now,
        }).await;
    }

    // Off the request path: a slow photo CDN must not hold up signing in.
    if let Some(picture) = claims.picture.clone() {
        let user_id = user_id.clone();
        actix_web::rt::spawn(async move { refresh_social_picture(&user_id, &picture).await });
    }

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

    // The same cookie session sign_in.rs sets, so server-rendered pages see
    // a Google sign-in exactly as they see a password one.
    session.insert("refresh_token", &refresh_token).ok();
    session.insert("user_id", &user_id).ok();
    session.insert("role", Account::AccountRole::User.to_string()).ok();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "access_token": access_token,
        "access_token_valid_till": access_token_valid_till,
        "refresh_token": refresh_token,
        "user_id": user_id,
        "role": "User",
    })))
}

/// The Firebase web config the sign-in page needs to open Google's popup.
/// None of it is secret — a web API key only identifies the project, and
/// Firebase ships it to every browser — but it lives in `.env` so a second
/// project (staging) is a config change rather than a code change. 404 until
/// it is set, which the page reads as "Google sign-in is off".
pub async fn web_config() -> Result<HttpResponse, Error> {
    let read = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());

    let (Some(api_key), Some(app_id), Some(project_id)) = (
        read("FIREBASE_WEB_API_KEY"),
        read("FIREBASE_APP_ID"),
        crate::Integrations::Firebase::project_id(),
    ) else {
        return Ok(Response::not_found("Google sign-in is not configured"));
    };
    let auth_domain = read("FIREBASE_AUTH_DOMAIN")
        .unwrap_or_else(|| format!("{}.firebaseapp.com", project_id));

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "apiKey": api_key,
        "authDomain": auth_domain,
        "projectId": project_id,
        "appId": app_id,
    })))
}

/// Larger than 5 MB is not a profile photo.
const MAX_PICTURE_BYTES: usize = 5 * 1024 * 1024;

/*
  Copies the provider's photo into our own image store and makes it the
  account's `social_picture`, replacing the copy from the previous sign-in.

  Copied rather than linked: Facebook's photo URLs are signed and expire
  within days, and linking either provider would also tell it every time a
  page showed the avatar. Runs on every social sign-in, so a photo changed
  on Google or Facebook shows here from the next sign-in on. Failures are
  logged and dropped - the old picture, or the placeholder, stays.
*/
async fn refresh_social_picture(user_id: &str, picture: &str) {
    let Some(url) = picture_url(picture) else {
        log::warn!("social picture: refusing {}", picture);
        return;
    };

    let client = match reqwest::Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(client) => client,
        Err(error) => { log::warn!("social picture client: {:?}", error); return; }
    };
    let bytes = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => { log::warn!("social picture read: {:?}", error); return; }
        },
        Ok(r) => { log::warn!("social picture fetch: HTTP {}", r.status()); return; }
        Err(error) => { log::warn!("social picture fetch: {:?}", error); return; }
    };
    if bytes.is_empty() || bytes.len() > MAX_PICTURE_BYTES {
        return;
    }

    let image_id = match crate::Handler::Image::Upload::store(&bytes, crate::model::AssetUsedAt::ProfilePic).await {
        Ok(id) => id,
        Err(error) => { log::warn!("social picture store: {}", error); return; }
    };

    let previous = MongoDB.connect()
        .collection::<Account::AccountProfile>("account_profile")
        .find_one_and_update(
            doc!{ "uuid": user_id },
            doc!{ "$set": { "social_picture": &image_id } },
        )
        .await;
    match previous {
        Ok(Some(profile)) => {
            if let Some(old) = profile.social_picture.filter(|old| old != &image_id) {
                crate::Handler::Image::Upload::discard(&old).await;
            }
        }
        // No profile row to hang it on, so the copy would be an orphan.
        Ok(None) => crate::Handler::Image::Upload::discard(&image_id).await,
        Err(error) => {
            log::error!("social picture save: {:?}", error);
            crate::Handler::Image::Upload::discard(&image_id).await;
        }
    }
}

/*
  The photo URL to fetch, or None for anything that is not one of the two
  providers' image hosts. The claim comes from a signed token, but the
  server should still never fetch an arbitrary address on its behalf.
  Asks each for a size fit for a profile page rather than its tiny default.
*/
fn picture_url(picture: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(picture).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();

    if host.ends_with(".googleusercontent.com") {
        // Google sizes by path suffix: ".../photo=s96-c".
        let path = url.path().to_string();
        if let Some(at) = path.rfind("=s") {
            url.set_path(&format!("{}=s400-c", &path[..at]));
        }
        Some(url)
    } else if host == "graph.facebook.com" {
        url.set_query(Some("width=400&height=400"));
        Some(url)
    } else {
        None
    }
}
