use chrono::Utc;
use uuid::Uuid;
use mongodb::bson::doc;
use serde::Deserialize;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Model::Account;
use actix_web::{web, Error, HttpResponse};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    email: String,
    password: String,
    reason: Option<String>,
    details: Option<String>,
}

/// Public account-deletion **request** endpoint backing the `/account/delete`
/// web page.
///
/// Anyone can hit this, so it re-authenticates with email + password (the same
/// check as sign-in) before recording anything. It does NOT delete the account
/// — it files a request that an admin reviews and actions from the dashboard
/// (policy: the account is removed within 15 days). Filing again just refreshes
/// the existing request (one per user).
pub async fn task(body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let email = body.email.trim().to_lowercase();
    let password = body.password.trim().to_string();

    if email.is_empty() {
        return Ok(Response::bad_request("Email is required"));
    }
    if password.is_empty() {
        return Ok(Response::bad_request("Password is required"));
    }

    let db = MongoDB.connect();

    // Only app users can self-delete from the web page (admins live elsewhere).
    let account = db
        .collection::<Account::AccountCore>("account_core")
        .find_one(doc! {
            "role": Account::AccountRole::User.to_string(),
            "email_address": &email,
        })
        .await;

    let account = match account {
        Ok(Some(acc)) => acc,
        Ok(None) => return Ok(Response::not_found("No account found for that email")),
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    };

    // Same credential check sign-in uses: the account must be verified and the
    // password must match before we accept the request.
    if !account.email_verified || account.password != password {
        return Ok(Response::forbidden("Incorrect email or password"));
    }

    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Not specified")
        .to_string();

    // Only keep free-text details when a reason of "Other" was given.
    let details = body
        .details
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(500).collect::<String>());

    let now = Utc::now().timestamp_millis();
    let request = Account::AccountDeletionRequest {
        uuid: Uuid::now_v7().to_string(),
        user_id: account.uuid.clone(),
        email: account.email_address.clone(),
        reason,
        details,
        requested_at: now,
    };

    // One request per user — a repeat submission replaces the previous one.
    let result = db
        .collection::<Account::AccountDeletionRequest>("account_deletion_request")
        .replace_one(doc! { "user_id": &account.uuid }, &request)
        .upsert(true)
        .await;

    if let Err(error) = result {
        log::error!("account deletion request failed: {:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message(
        "Your account deletion request has been received. Your account and all \
         personal data will be permanently deleted within 15 days.",
    ))
}
