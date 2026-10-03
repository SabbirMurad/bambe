use mongodb::bson::doc;
use crate::Model::Account;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use serde::{ Serialize, Deserialize };
use crate::utils::validation::validate_password;
use crate::utils::password::{self, Verified};
use actix_web::{ web, Error, HttpResponse, HttpRequest };
use crate::Middleware::Auth::{require_access, AccessRequirement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqBody {
    current_password: String,
    new_password: String,
    confirm_password: String,
}

pub async fn task(
    req: HttpRequest,
    body: web::Json<ReqBody>,
) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;

    let current_password = body.current_password.trim().to_string();
    let new_password = body.new_password.trim().to_string();
    let confirm_password = body.confirm_password.trim().to_string();

    if current_password.is_empty() {
        return Ok(Response::bad_request("Current password is required"));
    }

    // validate new password (length, charset, match)
    if let Err(error) = validate_password(&new_password, &confirm_password) {
        return Ok(Response::bad_request(&error));
    }

    if new_password == current_password {
        return Ok(Response::bad_request(
            "New password must be different from your current password"
        ));
    }

    let db = MongoDB.connect();
    let collection = db.collection::<Account::AccountCore>("account_core");

    let result = collection.find_one(doc!{ "uuid": &user_id }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let account_core = match result.unwrap() {
        Some(account) => account,
        None => return Ok(Response::not_found("Account not found")),
    };

    if password::verify(&current_password, &account_core.password) == Verified::Failed {
        return Ok(Response::forbidden("Current password is incorrect"));
    }

    /* Whatever the old row held, what replaces it is a hash — so this
       path also upgrades an account still stored the legacy way. */
    let hashed_password = match password::hash(&new_password) {
        Ok(h) => h,
        Err(error) => {
            log::error!("{error}");
            return Ok(Response::internal_server_error("Could not change the password"));
        }
    };

    let result = collection.update_one(
        doc!{ "uuid": &user_id },
        doc!{ "$set": { "password": &hashed_password } },
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("Successfully changed your password"))
}
