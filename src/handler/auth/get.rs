use mongodb::bson::doc;
use crate::{builtins::mongo::MongoDB, model, utils::response::Response};
use actix_web::{web, Error, HttpResponse};

pub async fn task(email_or_username: web::Path<String>,) -> Result<HttpResponse, Error> {
    let email_or_username = email_or_username.trim().to_string().to_lowercase();

    if email_or_username.len() == 0 {
        return Ok(Response::bad_request("Email/Username is required"));
    }

    let db = MongoDB.connect();
    let collection = db.collection::<model::Account::AccountCore>("account_core");

    let result = collection.find_one(
        doc!{"$and":[
            {"$or": [
                {"email_address": &email_or_username},
                {"username": &email_or_username},
            ]},
            {"email_verified": true}
        ]}
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::not_found(
            "No user found with this email or username"
        ));
    }

    let account_core = option.unwrap();

    let collection = db.collection::<model::Account::AccountProfile>("account_profile");

    let result = collection.find_one(
        doc!{"uuid": account_core.uuid}
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::not_found(
            "No user found with this email or username"
        ));
    }

    let account_profile = option.unwrap();

    let data = serde_json::json!({
        "first_name": account_profile.first_name,
        "last_name": account_profile.last_name,
        "profile_picture": account_profile.profile_picture,
        "username": account_core.username
    });

    Ok(HttpResponse::Ok().content_type("application/json").json(data))
}