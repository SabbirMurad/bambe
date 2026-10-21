use serde_json::json;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::{AccountCore, AccountProfile};
use crate::model::Customer::CustomerProfile;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let core = db.collection::<AccountCore>("account_core").find_one(doc! { "uuid": &user.user_id }).await.ok().flatten();
    let core = match core {
        Some(c) => c,
        None => return Ok(Response::not_found("Account not found")),
    };

    let profile = db.collection::<AccountProfile>("account_profile").find_one(doc! { "uuid": &user.user_id }).await.ok().flatten();
    let customer = db
        .collection::<CustomerProfile>("customer_profile")
        .find_one(doc! { "user_id": &user.user_id })
        .await
        .ok()
        .flatten();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "user_id": core.uuid,
        "email": core.email_address,
        "username": core.username,
        "email_verified": core.email_verified,
        "two_factor_enabled": core.two_a_factor_auth_enabled,
        "first_name": profile.as_ref().map(|p| p.first_name.clone()),
        "last_name": profile.as_ref().map(|p| p.last_name.clone()),
        "phone_number": profile.as_ref().and_then(|p| p.phone_number.clone()),
        "profile_picture": profile.as_ref().and_then(|p| p.profile_picture.clone()),
        "segment": customer.as_ref().map(|c| c.segment.clone()).unwrap_or_default(),
        "loyalty_points": customer.as_ref().map(|c| c.loyalty_points).unwrap_or(0),
        "accepts_marketing": customer.as_ref().map(|c| c.accepts_marketing).unwrap_or(false),
        "member_since": core.created_at,
    })))
}
