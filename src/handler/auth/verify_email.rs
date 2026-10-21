use chrono::Utc;
use mongodb::bson::doc;
use crate::{builtins::jwt, Model::Account};
use crate::BuiltIns::mongo::MongoDB;
use serde::{ Serialize, Deserialize };
use crate::util::response::Response;
use actix_web::{ web, Error, HttpResponse };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqBody {
    user_id: String,
    verification_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload {
    access_token: String,
    access_token_valid_till: i64,
    refresh_token: String,
    user_id: String,
    role: Account::AccountRole
}

pub async fn task(form_data: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let post_data = sanitize(&form_data);

    if let Err(res) = check_empty_fields(&post_data) {
        return Ok(Response::bad_request(&res));
    }

    /* DATABASE ACID SESSION INIT */
    let (db, mut session) = MongoDB.connect_acid().await;
    if let Err(error) = MongoDB::begin(&mut session).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    //checking if user exist
    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.find_one(
        doc!{"uuid": &post_data.user_id},
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        MongoDB::abort(&mut session).await;
        return Ok(Response::not_found("User not found"));
    }

    let account_core = option.unwrap();
    if account_core.email_verified {
        MongoDB::abort(&mut session).await;
        return Ok(Response::forbidden("User already verified"));
    }

    //check if Verification code match
    let collection = db.collection::
    <Account::AccountVerificationRequest>("account_verification_request");
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
        return Ok(Response::not_found("Verification request not found"));
    }

    let request = option.unwrap();
    if request.expires_at < Utc::now().timestamp_millis() {
        MongoDB::abort(&mut session).await;
        return Ok(Response::forbidden("Verification code expired"));
    }

    if request.attempts >= super::MAX_CODE_ATTEMPTS {
        MongoDB::abort(&mut session).await;
        return Ok(Response::forbidden("Too many incorrect attempts. Please request a new code."));
    }

    if request.code != post_data.verification_code {
        MongoDB::abort(&mut session).await;
        if super::record_wrong_code(&db, "account_verification_request", &request.uuid).await {
            return Ok(Response::forbidden("Too many incorrect attempts. Please request a new code."));
        }
        return Ok(Response::forbidden("Verification code incorrect"));
    }

    //make user verified
    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.update_one(
        doc!{ "uuid": &post_data.user_id },
        doc!{"$set": { "email_verified": true }},
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let count = result.unwrap().modified_count;
    if count == 0 {
        MongoDB::abort(&mut session).await;
        return Ok(Response::not_found("User not found"));
    }

    //delete Verification request
    let collection = db.collection
    ::<Account::AccountVerificationRequest>("account_verification_request");
    let result = collection.delete_many(
        doc!{"user_id": &post_data.user_id},
    ).session(&mut session).await;

    if let Err(error) = result {
      log::error!("{:?}", error);
      MongoDB::abort(&mut session).await;
      return Ok(Response::internal_server_error(&error.to_string()));
    }

    // getting access token
    let (access_token, valid_time) = match jwt::access_token::generate_default(&account_core.uuid, account_core.role.clone(),) {
        Ok(pair) => pair,
        Err(error) => return Ok(Response::internal_server_error(&format!("could not mint an access token: {:?}", error))),
    };

    // getting refresh token
    let result = jwt::refresh_token::new(&account_core.uuid);
    if let Err(error) = result {
        MongoDB::abort(&mut session).await;
        log::error!("{:?}", error); 
        return Ok(Response::internal_server_error(&error));
    }

    let refresh_token = result.unwrap();

    /* DATABASE ACID COMMIT */
    if let Err(error) = MongoDB::commit(&mut session).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let payload = Payload {
        access_token,
        access_token_valid_till: Utc::now().timestamp_millis() + (valid_time * 60 * 1000) as i64,
        refresh_token,
        user_id: account_core.uuid,
        role: account_core.role
    };
    Ok(HttpResponse::Ok().content_type("application/json").json(payload))
}

fn sanitize(form_data: &ReqBody) -> ReqBody {
  let mut form = form_data.clone();
  form.user_id = form.user_id.trim().to_string();
  form.verification_code = form.verification_code.trim().to_string();

  form
}

fn check_empty_fields(form_data: &ReqBody) -> Result<(), String> {
  if form_data.user_id.len() == 0 {
    Err("User id required".to_string())
  }
  else if form_data.verification_code.len() == 0 {
    Err("Verification code required".to_string())
  }
  else {
    Ok(())
  }
}