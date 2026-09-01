use rand::Rng;
use uuid::Uuid;
use chrono::Utc;
use mongodb::bson::doc;
use actix_session::Session;
use crate::{builtins::jwt, Model::Account};
use crate::Integrations::Resend;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use serde::{ Serialize, Deserialize };
use serde_json::json;
use mongodb::{ClientSession, Database};
use actix_web::{ web, Error, HttpResponse};

//in minutes
const CODE_EXPIRE_TIME: i64 = 15;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReqBody {
    email_or_username: String,
    password: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload {
    two_afa_enabled: bool,
    auth_payload: Option<AuthPayload>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthPayload {
    access_token: String,
    access_token_valid_till: i64,
    refresh_token: String,
    user_id: String,
    role: Account::AccountRole,
}

/// App sign-in — only matches `User` accounts (admins live in `admin_account`).
pub async fn task(form_data: web::Json<ReqBody>, actix_session: Session) -> Result<HttpResponse, Error> {
    perform(form_data, actix_session, Account::AccountRole::User).await
}

async fn perform(
    form_data: web::Json<ReqBody>,
    actix_session: Session,
    role: Account::AccountRole,
) -> Result<HttpResponse, Error> {
    let email_or_username = form_data.email_or_username.trim().to_string().to_lowercase();
    if email_or_username.len() == 0 {
        return Ok(Response::bad_request("Email/Username is required"));
    }

    let password = form_data.password.trim().to_string();
    if password.len() == 0 {
        return Ok(Response::bad_request("Password is required"));
    }

    /* DATABASE ACID SESSION INIT */
    let (db, mut session) = MongoDB.connect_acid().await;
    if let Err(error) = session.start_transaction().await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let account_core = match validate_login(
        &db,
        &mut session,
        &email_or_username,
        &password,
        &role,
    ).await {
        Ok(res) => res,
        Err(error) => {
            return Ok(error);
        }
    };

    // checking if 2FA is enabled
    if account_core.two_a_factor_auth_enabled {
        // Create a verification request
        let collection = db.collection::<Account::SignInVerificationRequest>("sign_in_verification_request");

        let mut rng = rand::rng();
        let validation_code: u32 = rng.random_range(100000..999999);

        let request_id = Uuid::now_v7().to_string();
        let now = Utc::now().timestamp_millis();
        let request = Account::SignInVerificationRequest {
            user_id: account_core.uuid.clone(),
            uuid:request_id.clone(),
            expires_at: now + CODE_EXPIRE_TIME * 60 * 1000,
            validation_code: validation_code.to_string(),
        };

        let result = collection.insert_one(
            request,
        ).session(&mut session).await;

        if let Err(error) = result {
            log::error!("{:?}", error);
            session.abort_transaction().await.ok().unwrap();
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        let message = Resend::sign_in_verification_code_template(
            &validation_code.to_string()
        );

        let result = Resend::send_email(
            &account_core.email_address,
            "Your Bambe sign-in code",
            &message
        ).await;
        if let Err(_) = result {
            session.abort_transaction().await.ok().unwrap();
            return Ok(Response::internal_server_error("Failed to send email"));
        }

        /*
          Stop here. This branch used to write the request, send the code and
          then fall straight through to minting a token — so enabling
          two-factor bought a spare email and no second factor at all. It went
          unnoticed because nothing could set the flag; the account settings
          can now, so it matters.

          The caller gets the request id and no token, and exchanges the two
          for one at `POST /v1/auth/verify-sign-in-code`.
        */
        if let Err(error) = session.commit_transaction().await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }

        return Ok(HttpResponse::Ok().content_type("application/json").json(json!({
            "two_afa_enabled": true,
            "auth_payload": serde_json::Value::Null,
            "request_id": request_id,
        })));
    }

    // getting access token
    let (access_token, valid_time) = jwt::access_token::generate_default(
        &account_core.uuid,
        account_core.role.clone(),
    );

    // getting refresh token
    let result = jwt::refresh_token::new(&account_core.uuid);
    if let Err(error) = result {
        session.abort_transaction().await.ok().unwrap();
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error));
    }

    let refresh_token = result.unwrap();


    /* DATABASE ACID COMMIT */
    if let Err(error) = session.commit_transaction().await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // setting session
    actix_session.insert("refresh_token", &refresh_token).unwrap();
    actix_session.insert("user_id", &account_core.uuid).unwrap();
    actix_session.insert("role", account_core.role.to_string()).unwrap();

    let data = AuthPayload {
        access_token,
        access_token_valid_till: Utc::now().timestamp_millis() + (valid_time * 60 * 1000) as i64,
        refresh_token,
        user_id: account_core.uuid.clone(),
        role: account_core.role.clone(),
    };

    let payload = Payload {
        two_afa_enabled: false,
        auth_payload: Some(data)
    };

    Ok(HttpResponse::Ok().content_type("application/json").json(payload))
}

// helper functions
async fn validate_login(
    db: &Database,
    session: &mut ClientSession,
    email_or_username: &str,
    password: &str,
    role: &Account::AccountRole,
) -> Result<Account::AccountCore, HttpResponse> {
    let collection = db.collection::<Account::AccountCore>("account_core");

    // Scope the lookup to the requested role so app and dashboard sign-in never
    // resolve to each other's account (an email may exist under both roles).
    let result = collection.find_one(
        doc!{
            "role": role.to_string(),
            "$or": [
                {"username": email_or_username},
                {"email_address": email_or_username},
            ],
        },
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let None = option {
        session.abort_transaction().await.ok().unwrap();
        return Err(Response::not_found("Username or email not found"));
    }

    let account_core = option.unwrap();

    // Accounts suspended from the admin dashboard cannot sign in.
    if account_core.suspended_at.is_some() {
        session.abort_transaction().await.ok().unwrap();
        return Err(Response::forbidden("This account has been suspended"));
    }

    if account_core.email_verified {
        if account_core.password != password {
            session.abort_transaction().await.ok().unwrap();
            return Err(Response::forbidden("Incorrect password"));
        }
        else {
            Ok(account_core)
        }
    }
    else {
        if let Err(error) = super::delete_account(
            db,
            session,
            &account_core.uuid
        ).await {
            Err(error)
        }
        else {
            session.abort_transaction().await.ok().unwrap();
            return Err(Response::not_found("Username or email not found"));
        }
    }
}
