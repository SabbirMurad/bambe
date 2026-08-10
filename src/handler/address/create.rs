use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Address::Address;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    label: String,
    recipient_name: String,
    street: String,
    city: String,
    state: Option<String>,
    zip: Option<String>,
    country: String,
    phone: String,
    note: Option<String>,
    #[serde(default)]
    is_default: bool,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<Address>("address");

    if body.label.trim().is_empty()
        || body.recipient_name.trim().is_empty()
        || body.street.trim().is_empty()
        || body.city.trim().is_empty()
        || body.country.trim().is_empty()
        || body.phone.trim().is_empty()
    {
        return Ok(Response::bad_request("Label, recipient, street, city, country and phone are required"));
    }

    let has_existing = col
        .count_documents(doc! { "user_id": &user.user_id, "deleted_at": null })
        .await
        .unwrap_or(0)
        > 0;
    // The first address in the book has nothing to default over.
    let is_default = body.is_default || !has_existing;

    if is_default {
        col.update_many(
            doc! { "user_id": &user.user_id, "deleted_at": null },
            doc! { "$set": { "is_default": false } },
        )
        .await
        .ok();
    }

    let now = Utc::now().timestamp_millis();
    let address = Address {
        uuid: Uuid::now_v7().to_string(),
        user_id: user.user_id,
        label: body.label.clone(),
        recipient_name: body.recipient_name.clone(),
        street: body.street.clone(),
        city: body.city.clone(),
        state: body.state.clone(),
        zip: body.zip.clone(),
        country: body.country.clone(),
        phone: body.phone.clone(),
        note: body.note.clone(),
        is_default,
        created_at: now,
        modified_at: now,
        deleted_at: None,
    };

    if let Err(error) = col.insert_one(&address).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(address))
}
