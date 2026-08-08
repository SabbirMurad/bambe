use chrono::Utc;
use mongodb::bson::doc;
use crate::Model::Metadata;
use crate::builtins::mongo::MongoDB;
use serde::{ Serialize, Deserialize };
use crate::utils::response::Response;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ReqBody {
    name: String,
    current_version_android: i64,
    last_supported_version_android: i64,
    emoji_pack_version: i64,
    under_maintenance: bool,
    description: String,
    developer: String,
    developer_email: String,
    developer_phone_number: Option<String>,
    terms_of_service: String,
    privacy_policy: String,
    community_guideline: String,
}

pub async fn task(req: HttpRequest, req_body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(
        &req,
        AccessRequirement::AnyToken
    )?;

    let user_id = user.user_id;

    let db = MongoDB.connect();

    let collection = db.collection::<Metadata::AppMetadata>("app_metadata");

    let result = collection.find_one(
        doc!{},
    ).await;
  
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }
    
    let option = result.unwrap();
    if let Some(_) = option {
        return Ok(Response::conflict(
            "App Metadata already exist"
        ));
    }

    let now = Utc::now().timestamp_millis();

    let app_metadata = Metadata::AppMetadata {
        name: req_body.name.clone(),
        description: req_body.description.clone(),

        developer: req_body.developer.clone(),
        developer_email: req_body.developer_email.clone(),
        developer_phone_number: req_body.developer_phone_number.clone(),

        emoji_pack_version: req_body.emoji_pack_version,

        last_supported_version_android: req_body.last_supported_version_android,
        current_version_android: req_body.current_version_android,

        terms_of_service: req_body.terms_of_service.clone(),
        privacy_policy: req_body.privacy_policy.clone(),
        community_guideline: req_body.community_guideline.clone(),

        under_maintenance: req_body.under_maintenance,
        created_at: now.clone(),
        created_by: user_id.clone(),
        updated_at : None,
        updated_by: None,
    };

    let result = collection.insert_one(
        app_metadata,
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(
        Response { message: "Successfully Created".to_string() }
    ))
}