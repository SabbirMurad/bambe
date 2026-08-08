use crate::Model::Metadata;
use chrono::Utc;
use mongodb::bson::{Document, doc};
use crate::builtins::mongo::MongoDB;
use serde::{ Serialize, Deserialize };
use crate::utils::response::Response;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReqBody {
    name: Option<String>,
    current_version_android: Option<i64>,
    last_supported_version_android: Option<i64>,
    emoji_pack_version: Option<i64>,
    under_maintenance: Option<bool>,
    description: Option<String>,
    developer: Option<String>,
    developer_email: Option<String>,
    developer_phone_number: Option<String>,
    terms_of_service: Option<String>,
    privacy_policy: Option<String>,
    community_guideline: Option<String>,
}

pub async fn task(req: HttpRequest, req_body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(
        &req,
        AccessRequirement::AnyToken
    )?;

    let user_id = user.user_id;

    let db = MongoDB.connect();

    let collection = db.collection::<Metadata::AppMetadata>("app_metadata");

    let mut update_doc = get_update_doc(req_body.clone());

    let now = Utc::now().timestamp_millis();
    update_doc.insert("updated_by", user_id);
    update_doc.insert("updated_by", now);

    let result = collection.update_one(
        doc!{},
        doc!{"$set": update_doc},
    ).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(
        Response { message: "Successfully Updated".to_string() }
    ))
}

fn get_update_doc(req_body: ReqBody) -> Document {
    let mut doc = Document::new();

    if let Some(name) = req_body.name {
        doc.insert("name", name);
    }

    if let Some(current_version_android) = req_body.current_version_android {
        doc.insert("current_version_android", current_version_android);
    }

    if let Some(last_supported_version_android) = req_body.last_supported_version_android {
        doc.insert("last_supported_version_android", last_supported_version_android);
    }

    if let Some(emoji_pack_version) = req_body.emoji_pack_version {
        doc.insert("emoji_pack_version", emoji_pack_version);
    }

    if let Some(under_maintenance) = req_body.under_maintenance {
        doc.insert("under_maintenance", under_maintenance);
    }

    if let Some(description) = req_body.description {
        doc.insert("description", description);
    }

    if let Some(developer) = req_body.developer {
        doc.insert("developer", developer);
    }

    if let Some(developer_email) = req_body.developer_email {
        doc.insert("developer_email", developer_email);
    }

    if let Some(developer_phone_number) = req_body.developer_phone_number {
        doc.insert("developer_phone_number", developer_phone_number);
    }

    if let Some(terms_of_service) = req_body.terms_of_service {
        doc.insert("terms_of_service", terms_of_service);
    }

    if let Some(privacy_policy) = req_body.privacy_policy {
        doc.insert("privacy_policy", privacy_policy);
    }

    if let Some(community_guideline) = req_body.community_guideline {
        doc.insert("community_guideline", community_guideline);
    }

    doc
}