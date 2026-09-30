use std::env;
use uuid::Uuid;
use chrono::Utc;
use mongodb::bson::doc;
use crate::Model::Account;
use crate::BuiltIns::mongo::MongoDB;
use serde::{ Serialize, Deserialize };
use actix_web::{ web, Error, HttpResponse };
use crate::utils::response::Response;
use crate::utils::validation::{validate_email, validate_password};
use crate::utils::password;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqBody {
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    email_address: String,
    password: String,
    confirm_password: String,
    secret_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Payload { user_id: String }

/// Creates an Administrator account in the dedicated `admin_account` collection.
/// Gated by ADMIN_SIGNUP_SECRET. Admins are entirely separate from app users,
/// so an admin email may also exist as a separate app account.
pub async fn task(form_data: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    /*
      This endpoint is unauthenticated — an existing admin is not required to
      create another one — so this secret is the whole of its access control.
      The value shipped in .env.example is refused outright: it is public, and
      a deployment still carrying it would let anyone who has read the repo
      make themselves an administrator.
    */
    const PLACEHOLDER: &str = "change-me-to-a-long-random-string";
    const MIN_SECRET_LEN: usize = 24;

    let secret = match env::var("ADMIN_SIGNUP_SECRET") {
        Ok(s) if !s.trim().is_empty() => s,
        _ => {
            log::error!("ADMIN_SIGNUP_SECRET is not set; refusing admin sign-up");
            return Ok(Response::internal_server_error("Admin sign-up is not configured"));
        }
    };
    if secret.trim() == PLACEHOLDER {
        log::error!(
            "ADMIN_SIGNUP_SECRET is still the example value; refusing admin sign-up. \
             Set it to a long random string."
        );
        return Ok(Response::internal_server_error("Admin sign-up is not configured"));
    }
    if secret.trim().len() < MIN_SECRET_LEN {
        log::error!(
            "ADMIN_SIGNUP_SECRET is shorter than {MIN_SECRET_LEN} characters; refusing admin sign-up"
        );
        return Ok(Response::internal_server_error("Admin sign-up is not configured"));
    }
    if form_data.secret_key.trim() != secret.trim() {
        return Ok(Response::forbidden("Invalid secret key"));
    }

    let email_address = form_data.email_address.trim().to_lowercase();
    let password = form_data.password.trim().to_string();
    let confirm_password = form_data.confirm_password.trim().to_string();
    let name = format!("{} {}", form_data.first_name.trim(), form_data.last_name.trim())
        .trim()
        .to_string();

    if email_address.is_empty() {
        return Ok(Response::bad_request("Email is required"));
    }
    if password.is_empty() {
        return Ok(Response::bad_request("Password is required"));
    }
    if let Err(res) = validate_password(&password, &confirm_password) {
        return Ok(Response::bad_request(&res));
    }
    if let Err(error) = validate_email(&email_address) {
        return Ok(Response::bad_request(&error));
    }

    let db = MongoDB.connect();
    let collection = db.collection::<Account::AdminAccount>("admin_account");

    // Email must be unique among admins (also enforced by a unique index).
    match collection.find_one(doc!{ "email_address": &email_address }).await {
        Ok(Some(_)) => return Ok(Response::bad_request("An admin with this email already exists")),
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
        Ok(None) => {}
    }

    let hashed_password = match password::hash(&password) {
        Ok(h) => h,
        Err(error) => {
            log::error!("{error}");
            return Ok(Response::internal_server_error("Could not create the account"));
        }
    };

    let admin = Account::AdminAccount {
        uuid: Uuid::now_v7().to_string(),
        email_address,
        name: if name.is_empty() { "Admin".to_string() } else { name },
        password: hashed_password,
        created_at: Utc::now().timestamp_millis(),
    };
    let user_id = admin.uuid.clone();

    if let Err(error) = collection.insert_one(admin).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(Payload { user_id }))
}
