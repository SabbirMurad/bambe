use rand::Rng;
use uuid::Uuid;
use chrono::Utc;
use mongodb::bson::doc;
use crate::Model::Account;
use crate::Integrations::Resend;
use crate::BuiltIns::mongo::MongoDB;
use serde::{ Serialize, Deserialize };
use crate::utils::response::Response;
use actix_web::{ web, Error, HttpResponse };

//time in minutes
const REQ_EXP_TIME: i64 = 15;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReqBody { email_or_username: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload { user_id: String, expires_in: i64 }

pub async fn task(form_data: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let email_or_username = form_data.email_or_username.trim().to_string().to_lowercase();

    /* DATABASE ACID SESSION INIT */
    let (db, mut session) = MongoDB.connect_acid().await;
    if let Err(error) = session.start_transaction().await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // Resolve the verified account by email or username.
    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.find_one(
        doc! {
            "$and": [
                { "$or": [
                    { "email_address": &email_or_username },
                    { "username": &email_or_username },
                ]},
                { "email_verified": true },
            ]
        },
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let None = option {
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::not_found(
            "No account found with this email or username"
        ));
    }

    let account_core = option.unwrap();

    //checking if reset request exist
    let collection = db.collection::
    <Account::PasswordResetRequest>("password_reset_request");
    let result = collection.count_documents(
        doc!{"user_id": &account_core.uuid},
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let count = result.unwrap();

    //creating validation request
    let mut rng = rand::rng();
    let validation_code: u32 = rng.random_range(100000..1000000);

    let secret_key = Uuid::now_v7().to_string() + "-" + &Uuid::now_v7().to_string() + "-" + &Uuid::now_v7().to_string();
    let expires_at = Utc::now().timestamp_millis() + (1000 * 60 * REQ_EXP_TIME);

    if count != 0 {
        //updating
        let result = collection.update_one(
            doc!{"user_id": &account_core.uuid},
            doc!{"$set": {
                "secret_key": &secret_key,
                "validation_code": &validation_code.to_string(),
                "expires_at": &expires_at
            }},
        ).session(&mut session).await;

        if let Err(error) = result {
            log::error!("{:?}", error);
            session.abort_transaction().await.ok().unwrap();
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        let message = Resend::password_reset_verification_code_template(
            &validation_code.to_string()
        );

        let result = Resend::send_email(
            &account_core.email_address,
            "Reset your Bambe password",
            &message
        ).await;
        if let Err(_) = result {
            session.abort_transaction().await.ok().unwrap();
            return Ok(Response::internal_server_error("Failed to send email"));
        }
    
        /* DATABASE ACID COMMIT */
        if let Err(error) = session.commit_transaction().await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        let data = Payload {
            user_id: account_core.uuid.clone(),
            expires_in: REQ_EXP_TIME
        };

        return Ok(
            HttpResponse::Ok()
            .content_type("application/json")
            .json(data)
        )
    }

    //creating the reset collection
    let collection = db.collection("password_reset_request");

    let result = collection.insert_one(
        doc!{
            "uuid": Uuid::now_v7().to_string(),
            "user_id": &account_core.uuid,
            "secret_key": &secret_key,
            "validation_code": &validation_code.to_string(),
            "code_validated": false,
            "expires_at": expires_at,
        },
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let message = Resend::password_reset_verification_code_template(
        &validation_code.to_string()
    );

    let result = Resend::send_email(
        &account_core.email_address,
        "Reset your Bambe password",
        &message
    ).await;
    if let Err(_) = result {
        session.abort_transaction().await.ok().unwrap();
        return Ok(Response::internal_server_error("Failed to send email"));
    }

    /* DATABASE ACID COMMIT */
    if let Err(error) = session.commit_transaction().await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let data = Payload {
        user_id: account_core.uuid.clone(),
        expires_in: REQ_EXP_TIME
    };
    
    Ok(HttpResponse::Ok().content_type("application/json").json(data))
}