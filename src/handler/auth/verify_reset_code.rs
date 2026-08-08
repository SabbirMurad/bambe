use chrono::Utc;
use mongodb::bson::doc;
use crate::Model::Account;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use serde::{ Serialize, Deserialize };
use actix_web::{ web, Error, HttpResponse };

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateResetFormData {
    user_id: String,
    validation_code: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReqBody { secret_key: String }

pub async fn task(form_data: web::Json<ValidateResetFormData>) -> Result<HttpResponse, Error> {
    let user_id = form_data.user_id.trim().to_string();
    let validation_code = form_data.validation_code.trim().to_string();

    if user_id.len() == 0 {
        return Ok(Response::bad_request("User Id required"));
    }

    if validation_code.len() == 0 {
        return Ok(Response::bad_request("Validation code required"));
    }

    /* DATABASE ACID SESSION INIT */
    let (db, mut session) = MongoDB.connect_acid().await;
    if let Err(error) = session.start_transaction().await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    //checking if email exist
    let collection = db.collection::<Account::AccountCore>("account_core");

    let result = collection.find_one(
        doc!{"uuid": user_id},
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let None = option {
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::not_found("Account not found"));
    }

    let user = option.unwrap();

    //checking if reset request exist
    let collection = db.collection::
    <Account::PasswordResetRequest>("password_reset_request");
    let result = collection.find_one(
        doc!{"user_id": &user.uuid},
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let None = option {
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::not_found("Reset request not found"));
    }

    let reset_request = option.unwrap();
    if reset_request.expires_at < Utc::now().timestamp_millis() {
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::not_found("Reset request expired"));
    }

    if reset_request.validation_code != validation_code {
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::forbidden("Validation Code Invalid"));
    }

    //update reset collection
    let collection = db.collection::
    <Account::PasswordResetRequest>("password_reset_request");

    let result = collection.update_one(
        doc!{ "uuid": reset_request.uuid },
        doc!{"$set": { "code_validated": true }},
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    /* DATABASE ACID COMMIT */
    if let Err(error) = session.commit_transaction().await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let data = ReqBody {secret_key: reset_request.secret_key};

    Ok(HttpResponse::Ok().content_type("application/json").json(data))
}