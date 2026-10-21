use chrono::Utc;
use mongodb::bson::doc;
use actix_session::Session;
use crate::{builtins::jwt, Model::Account};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::util::password::{self, Verified};
use serde::{ Serialize, Deserialize };
use actix_web::{ web, Error, HttpResponse };

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    email_or_username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthPayload {
    access_token: String,
    access_token_valid_till: i64,
    refresh_token: String,
    user_id: String,
    role: Account::AccountRole,
}

#[derive(Debug, Serialize)]
struct Payload {
    two_afa_enabled: bool,
    auth_payload: AuthPayload,
}

/// Dashboard sign-in — authenticates against the `admin_account` collection
/// only. App users (account_core) can't sign in here, and vice-versa.
pub async fn task(form_data: web::Json<ReqBody>, actix_session: Session) -> Result<HttpResponse, Error> {
    let email = form_data.email_or_username.trim().to_lowercase();
    if email.is_empty() {
        return Ok(Response::bad_request("Email is required"));
    }
    let password = form_data.password.trim().to_string();
    if password.is_empty() {
        return Ok(Response::bad_request("Password is required"));
    }

    let db = MongoDB.connect();
    let collection = db.collection::<Account::AdminAccount>("admin_account");
    let admin = match collection
        .find_one(doc!{ "email_address": &email })
        .await
    {
        Ok(Some(a)) => a,
        Ok(None) => return Ok(Response::not_found("Admin account not found")),
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    };

    match password::verify(&password, &admin.password) {
        Verified::Failed => return Ok(Response::forbidden("Incorrect password")),
        // Legacy row, correct password — upgrade it in place. See sign_in.rs.
        Verified::NeedsRehash => {
            match password::hash(&password) {
                Ok(hashed) => {
                    let update = collection.update_one(
                        doc!{ "uuid": &admin.uuid },
                        doc!{ "$set": { "password": &hashed } },
                    ).await;
                    if let Err(error) = update {
                        log::error!("could not upgrade stored password: {:?}", error);
                    }
                }
                Err(error) => log::error!("{error}"),
            }
        }
        Verified::Ok => {}
    }

    let (access_token, valid_time) = match jwt::access_token::generate_default(&admin.uuid, Account::AccountRole::Administrator) {
        Ok(pair) => pair,
        Err(error) => return Ok(Response::internal_server_error(&format!("could not mint an access token: {:?}", error))),
    };

    let refresh_token = match jwt::refresh_token::new(&admin.uuid) {
        Ok(token) => token,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error));
        }
    };

    actix_session.insert("refresh_token", &refresh_token).ok();
    actix_session.insert("user_id", &admin.uuid).ok();
    actix_session.insert("role", "Administrator").ok();

    let payload = Payload {
        two_afa_enabled: false,
        auth_payload: AuthPayload {
            access_token,
            access_token_valid_till: Utc::now().timestamp_millis() + (valid_time * 60 * 1000) as i64,
            refresh_token,
            user_id: admin.uuid.clone(),
            role: Account::AccountRole::Administrator,
        },
    };

    Ok(HttpResponse::Ok().content_type("application/json").json(payload))
}
