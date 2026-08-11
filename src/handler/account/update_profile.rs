use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountProfile;
use crate::model::Customer::CustomerProfile;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    first_name: Option<String>,
    last_name: Option<String>,
    phone_number: Option<String>,
    profile_picture: Option<String>,
    accepts_marketing: Option<bool>,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();

    let profile_col = db.collection::<AccountProfile>("account_profile");
    let existing_profile = profile_col.find_one(doc! { "uuid": &user.user_id }).await.ok().flatten();
    let is_new = existing_profile.is_none();

    // `first_name`/`last_name` are required fields on the model, so the
    // very first save has to land a complete document even if the caller
    // only touched one field this time.
    let mut set_doc = doc! { "modified_at": now };
    match &body.first_name {
        Some(v) => { set_doc.insert("first_name", v); }
        None if is_new => { set_doc.insert("first_name", ""); }
        None => {}
    }
    match &body.last_name {
        Some(v) => { set_doc.insert("last_name", v); }
        None if is_new => { set_doc.insert("last_name", ""); }
        None => {}
    }
    if let Some(v) = &body.phone_number { set_doc.insert("phone_number", v); }
    if let Some(v) = &body.profile_picture { set_doc.insert("profile_picture", v); }

    let result = profile_col
        .update_one(
            doc! { "uuid": &user.user_id },
            doc! { "$set": set_doc, "$setOnInsert": { "uuid": &user.user_id } },
        )
        .upsert(true)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    if let Some(accepts_marketing) = body.accepts_marketing {
        let customer_col = db.collection::<CustomerProfile>("customer_profile");
        let result = customer_col
            .update_one(
                doc! { "user_id": &user.user_id },
                doc! {
                    "$set": { "accepts_marketing": accepts_marketing, "marketing_updated_at": now, "modified_at": now },
                    "$setOnInsert": {
                        "uuid": uuid::Uuid::now_v7().to_string(),
                        "user_id": &user.user_id,
                        "segment": "New",
                        "order_count": 0,
                        "lifetime_spend": 0,
                        "loyalty_points": 0,
                        "tags": Vec::<String>::new(),
                        "created_at": now,
                    },
                },
            )
            .upsert(true)
            .await;

        if let Err(error) = result {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    Ok(Response::ok_message("Profile updated"))
}
