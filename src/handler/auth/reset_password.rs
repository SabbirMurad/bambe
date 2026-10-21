use chrono::Utc;
use mongodb::bson::doc;
use crate::Model::Account;
use crate::BuiltIns::mongo::MongoDB;
use crate::Util::response::Response;
use serde::{ Serialize, Deserialize };
use actix_web::{ web, Error, HttpResponse };
use crate::Util::validation::validate_password;
use crate::Util::password;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPasswordFormData {
    user_id: String,
    secret_key: String,
    new_password: String,
    confirm_password: String,
}

pub async fn task(form_data: web::Json<ResetPasswordFormData>) -> Result<HttpResponse, Error> {
    let post_data = sanitize(&form_data);

    if let Err(error) = check_empty_fields(&post_data) {
        return Ok(Response::bad_request(&error));
    }

    //validate password
    if let Err(res) = validate_password(&post_data.new_password, &post_data.confirm_password) {
        return Ok(Response::bad_request(&res));
    }

    /* DATABASE ACID SESSION INIT */

    let (db, mut session) = MongoDB.connect_acid().await;
    if let Err(error) = MongoDB::begin(&mut session).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    //checking if reset request exist
    let collection = db.collection::
    <Account::PasswordResetRequest>("password_reset_request");

    let result = collection.find_one(
        doc!{"user_id": &post_data.user_id},
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let None = option {
        MongoDB::abort(&mut session).await;
        return Ok(Response::not_found("reset request does not exist"));
    }

    //validating
    let reset = option.unwrap();

    if reset.expires_at < Utc::now().timestamp_millis() {
        MongoDB::abort(&mut session).await;
        return Ok(Response::forbidden("request time expired"));
    }

    if !reset.code_validated {
        MongoDB::abort(&mut session).await;
        return Ok(Response::forbidden("Validation code not validated"));
    }

    if reset.secret_key != post_data.secret_key {
        MongoDB::abort(&mut session).await;
        return Ok(Response::forbidden("Secret key invalid"));
    }

    //resetting password
    let hashed_password = match password::hash(&post_data.new_password) {
        Ok(h) => h,
        Err(error) => {
            log::error!("{error}");
            MongoDB::abort(&mut session).await;
            return Ok(Response::internal_server_error("Could not reset the password"));
        }
    };

    let collection = db.collection::<Account::AccountCore>("account_core");

    let result = collection.update_one(
        doc!{"uuid": &post_data.user_id},
        doc!{"$set": {
            "password": &hashed_password
        }},
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    //deleting reset request
    let collection = db.collection::
    <Account::PasswordResetRequest>("password_reset_request");

    let result = collection.delete_one(
        doc!{"uuid": &reset.uuid, "user_id": &post_data.user_id},
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    /* DATABASE ACID COMMIT */
    if let Err(error) = MongoDB::commit(&mut session).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(
        Response { message: "Successfully reset your password".to_string()}
    ))
}

fn sanitize(form_data: &ResetPasswordFormData) -> ResetPasswordFormData {
    let mut form = form_data.clone();
    form.user_id = form.user_id.trim().to_string();
    form.secret_key = form.secret_key.trim().to_string();
    form.new_password = form.new_password.trim().to_string();
    form.confirm_password = form.confirm_password.trim().to_string();

    return form;
}

fn check_empty_fields(post_data: &ResetPasswordFormData) -> Result<(), String> {
    if post_data.user_id.len() == 0 {
        return Err("User id required".to_string());
    }
    else if post_data.secret_key.len() == 0 {
        return Err("Secret key required".to_string());
    }
    else if post_data.new_password.len() == 0 {
        return Err("new password required".to_string());
    }
    else if post_data.confirm_password.len() == 0 {
        return Err("confirm password required".to_string());
    }
    else {
        Ok(())
    }
}