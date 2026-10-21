//! Second step of a two-step sign-in.
//!
//! `sign_in` stops when `two_a_factor_auth_enabled` is set: it writes a
//! `sign_in_verification_request`, emails the code, and answers with the
//! request id and **no token**. This is what exchanges that id plus the code
//! for the token.
//!
//! Until this existed, `sign_in`'s 2FA branch wrote the request, sent the
//! email and then fell through to mint a token anyway — so the flag bought a
//! spare email and no security at all. It was invisible because nothing could
//! set the flag; the moment the account settings could, it mattered.

use chrono::Utc;
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};
use actix_web::{web, Error, HttpResponse};
use actix_session::Session;
use crate::Model::Account;
use crate::BuiltIns::mongo::MongoDB;
use crate::builtins::jwt;
use crate::util::response::Response;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    request_id: String,
    validation_code: String,
}

#[derive(Debug, Serialize)]
struct AuthPayload {
    access_token: String,
    access_token_valid_till: i64,
    refresh_token: String,
    user_id: String,
    role: Account::AccountRole,
}

pub async fn task(form_data: web::Json<ReqBody>, actix_session: Session) -> Result<HttpResponse, Error> {
    let request_id = form_data.request_id.trim().to_string();
    let validation_code = form_data.validation_code.trim().to_string();

    if request_id.is_empty() {
        return Ok(Response::bad_request("Request id required"));
    }
    if validation_code.is_empty() {
        return Ok(Response::bad_request("Validation code required"));
    }

    let db = MongoDB.connect();

    let request = db
        .collection::<Account::SignInVerificationRequest>("sign_in_verification_request")
        .find_one(doc! { "uuid": &request_id })
        .await
        .ok()
        .flatten();

    let request = match request {
        Some(request) => request,
        // The same answer for an unknown id and a wrong code, so neither can
        // be probed to learn about the other.
        None => return Ok(Response::forbidden("That code isn't right, or it has expired")),
    };

    let now = Utc::now().timestamp_millis();
    if request.expires_at < now {
        // Spent either way — an expired request must not survive to be retried.
        db.collection::<Account::SignInVerificationRequest>("sign_in_verification_request")
            .delete_one(doc! { "uuid": &request_id })
            .await
            .ok();
        return Ok(Response::forbidden("That code isn't right, or it has expired"));
    }

    if request.attempts >= super::MAX_CODE_ATTEMPTS {
        return Ok(Response::forbidden("Too many incorrect attempts. Please request a new code."));
    }

    if request.validation_code != validation_code {
        if super::record_wrong_code(&db, "sign_in_verification_request", &request.uuid).await {
            return Ok(Response::forbidden("Too many incorrect attempts. Please request a new code."));
        }
        return Ok(Response::forbidden("That code isn't right, or it has expired"));
    }

    let account = db
        .collection::<Account::AccountCore>("account_core")
        .find_one(doc! { "uuid": &request.user_id })
        .await
        .ok()
        .flatten();

    let account = match account {
        Some(account) => account,
        None => return Ok(Response::not_found("Account not found")),
    };

    // Re-checked here, not just at step one: an account suspended between the
    // two steps must not be able to finish signing in.
    if account.suspended_at.is_some() {
        return Ok(Response::forbidden("This account has been suspended"));
    }

    /*
      One code, one token. Deleting before minting means a replay of the same
      request id finds nothing, so an intercepted code cannot be used twice.
    */
    let spent = db
        .collection::<Account::SignInVerificationRequest>("sign_in_verification_request")
        .delete_one(doc! { "uuid": &request_id })
        .await;

    match spent {
        Ok(result) if result.deleted_count == 0 => {
            return Ok(Response::forbidden("That code isn't right, or it has expired"));
        }
        Ok(_) => {}
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    let (access_token, valid_time) = match jwt::access_token::generate_default(&account.uuid, account.role.clone()) {
        Ok(pair) => pair,
        Err(error) => return Ok(Response::internal_server_error(&format!("could not mint an access token: {:?}", error))),
    };

    let refresh_token = match jwt::refresh_token::new(&account.uuid) {
        Ok(token) => token,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error));
        }
    };

    actix_session.insert("refresh_token", &refresh_token).unwrap();
    actix_session.insert("user_id", &account.uuid).unwrap();
    actix_session.insert("role", account.role.to_string()).unwrap();

    Ok(HttpResponse::Ok().content_type("application/json").json(AuthPayload {
        access_token,
        access_token_valid_till: now + (valid_time * 60 * 1000) as i64,
        refresh_token,
        user_id: account.uuid,
        role: account.role,
    }))
}
