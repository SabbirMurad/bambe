use rand::Rng;
use uuid::Uuid;
use chrono::Utc;
use mongodb::bson::doc;
use crate::Model::Account;
use crate::Integrations::Resend;
use crate::BuiltIns::mongo::MongoDB;
use serde::{ Serialize, Deserialize };
use mongodb::{ClientSession, Database};
use actix_web::{ web, Error, HttpResponse };
use crate::util::response::Response;
use crate::util::validation::{validate_email, validate_password, validate_username};
use crate::util::password;


//in minutes
const CODE_EXPIRE_TIME: i64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqBody {
    // Name and username are collected later (during onboarding), so they are
    // optional at sign-up. When omitted, a placeholder username is generated.
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    #[serde(default)]
    username: String,
    email_address: String,
    password: String,
    confirm_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload { user_id: String }


pub async fn task(form_data: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let post_data = sanitize(&form_data);

    if let Err(res) = check_empty_fields(&post_data) {
        return Ok(Response::bad_request(&res));
    }

    if let Err(res) = validate_password(&post_data.password, &post_data.confirm_password) {
        return Ok(Response::bad_request(&res));
    }

    // Hashed here rather than at the insert below, so the plaintext never
    // reaches the document that gets written.
    let hashed_password = match password::hash(&post_data.password) {
        Ok(h) => h,
        Err(error) => {
            log::error!("{error}");
            return Ok(Response::internal_server_error("Could not create the account"));
        }
    };

    if let Err(error) = validate_email(&post_data.email_address) {
        return Ok(Response::bad_request(&error));
    }

    // Resolve the username: validate the one provided, otherwise generate a
    // unique placeholder (the user has no username field in the app UI).
    let username = if post_data.username.is_empty() {
        format!("user_{}", &Uuid::now_v7().simple().to_string()[..12])
    } else {
        if let Err(res) = validate_username(&post_data.username) {
            return Ok(Response::bad_request(&res));
        }
        post_data.username.clone()
    };

    /* DATABASE ACID SESSION INIT */

    let (db, mut session) = MongoDB.connect_acid().await;
    if let Err(error) = MongoDB::begin(&mut session).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    //check if user already exist
    if let Err(error) = account_already_exist(
        &db,
        &mut session,
        &username,
        &post_data.email_address
    ).await {
        return Ok(error);
    }

    //creating account_core
    let now = Utc::now().timestamp_millis();
    let user_id: String = Uuid::now_v7().to_string();

    let account_core = Account::AccountCore {
        uuid: user_id.clone(),
        email_address: post_data.email_address.clone(),
        username: username.clone(),
        password: hashed_password,
        email_verified: false,
        two_a_factor_auth_enabled: false,
        two_a_factor_auth_updated: None,
        role: Account::AccountRole::User,
        suspended_at: None,
        suspended_by: None,
        created_at: now,
    };

    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.insert_one(
        account_core,
    ).session(&mut session).await;

    if let Err(error) = result {
      log::error!("{:?}", error);
      MongoDB::abort(&mut session).await;
      return Ok(Response::internal_server_error(&error.to_string()));
    }

    //creating account_profile
    let collection = db.collection::
    <Account::AccountProfile>("account_profile");

    let account_profile = Account::AccountProfile {
        uuid: user_id.clone(),
        first_name: post_data.first_name.clone(),
        last_name: post_data.last_name.clone(),
        profile_picture: None,
        social_picture: None,
        phone_number: None,
        modified_at: now,
    };

    let result = collection.insert_one(
        account_profile,
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }
  
    //Create account_status

    //creating validation request
    let mut rng = rand::rng();
    let validation_code: u32 = rng.random_range(100000..999999);
    let request = Account::AccountVerificationRequest {
        uuid: Uuid::now_v7().to_string(),
        user_id: user_id.clone(),
        code: validation_code.to_string(),
        expires_at: now + CODE_EXPIRE_TIME * 60 * 1000,
        attempts: 0,
    };
    
    let collection = db.collection::
    <Account::AccountVerificationRequest>("account_verification_request");
    let result = collection.insert_one(
        request,
    ).session(&mut session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let message = Resend::sign_up_verification_code_template(
        &validation_code.to_string()
    );

    let result = Resend::send_email(
        &post_data.email_address,
        "Verify your account",
        &message
    ).await;

    if let Err(_) = result {
        MongoDB::abort(&mut session).await;
        return Ok(Response::internal_server_error("Failed to send email"));
    }

    /* DATABASE ACID COMMIT */
    if let Err(error) = MongoDB::commit(&mut session).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let data = Payload { user_id };
    Ok(HttpResponse::Ok().content_type("application/json").json(data))
}

async fn account_already_exist(
    db: &Database,
    session: &mut ClientSession,
    username: &str,
    email_address: &str
) -> Result<(), HttpResponse> {
    // Only a *User* account makes an email "taken" — an Administrator account
    // may share the email, and the app user can still register their own.
    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.find_one(
      doc!{"email_address": email_address, "role": "User"},
    ).session(&mut *session).await;

    if let Err(error) = result {
      log::error!("{:?}", error);
      MongoDB::abort(session).await;
      return Err(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let Some(account_core) = option {
        if account_core.email_verified {
            MongoDB::abort(session).await;
            return Err(Response::bad_request(
                "An account with this email already exist"
            ));
        }
        else {
            if let Err(error) = delete_account(
                db,
                session,
                &account_core.uuid
            ).await {
                return Err(error);
            }

            return Ok(());
        }
    }

    let collection = db.collection::
    <Account::AccountProfile>("account_profile");
    let result = collection.find_one(
      doc!{"username": username},
    ).session(&mut *session).await;

    if let Err(error) = result {
      log::error!("{:?}", error);
      MongoDB::abort(session).await;
      return Err(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();

    if let Some(account_profile) = option {
        let collection = db.collection::<Account::AccountCore>("account_core");
        let result = collection.find_one(
          doc!{"uuid": &account_profile.uuid},
        ).session(&mut *session).await;
    
        if let Err(error) = result {
          log::error!("{:?}", error);
          MongoDB::abort(session).await;
          return Err(Response::internal_server_error(&error.to_string()));
        }
    
        let option = result.unwrap();
        let account_core = option.unwrap();

        if account_core.email_verified {
            MongoDB::abort(session).await;
            return Err(Response::bad_request(
                "An account with this email already exist"
            ));
        }
        else {
            if let Err(error) = delete_account(
                db,
                session,
                &account_profile.uuid
            ).await {
                return Err(error);
            }

            return Ok(());
        }
    }

    return Ok(());
}

async fn delete_account(
    db: &Database,
    session: &mut ClientSession,
    user_id: &str
) -> Result<(), HttpResponse> {
    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(session).await;
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let collection = db.collection::
    <Account::AccountProfile>("account_profile");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(session).await;
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let collection = db.collection::
    <Account::AccountVerificationRequest>("account_verification_request");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(session).await;
        return Err(Response::internal_server_error(&error.to_string()));
    }

    Ok(())
}

fn sanitize(form_data: &ReqBody) -> ReqBody {
    let mut form = form_data.clone();
    form.password = form.password.trim().to_string();
    form.email_address = form.email_address.trim().to_string().to_lowercase();
    form.first_name = form.first_name.trim().to_string();
    form.last_name = form.last_name.trim().to_string();
    form.username = form.username.trim().to_string().to_lowercase();
    form.confirm_password = form.confirm_password.trim().to_string();
    
    form
}

fn check_empty_fields(form_data: &ReqBody) -> Result<(), String> {
    if form_data.password.len() == 0 {
        Err("Password is required".to_string())
    }
    else if form_data.email_address.len() == 0 {
        Err("Email is required".to_string())
    }
    else {
        Ok(())
    }
}