use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Order::DeliveryMethod;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: String,
    #[serde(default)]
    description: String,
    price: Cents,
    // Waives `price` once the subtotal clears it — the "free over $50" rule.
    free_above: Option<Cents>,
    min_days: u8,
    max_days: u8,
    // Minutes past midnight, for a same-day cutoff. None when there isn't one.
    cutoff_minutes: Option<u16>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    position: u16,
}
fn default_true() -> bool { true }

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    if let Err(message) = validate(&body) {
        return Ok(Response::bad_request(&message));
    }

    let method = DeliveryMethod {
        uuid: Uuid::now_v7().to_string(),
        name: body.name.trim().to_string(),
        description: body.description.trim().to_string(),
        price: body.price,
        free_above: body.free_above,
        min_days: body.min_days,
        max_days: body.max_days,
        cutoff_minutes: body.cutoff_minutes,
        enabled: body.enabled,
        position: body.position,
    };

    if let Err(error) = db.collection::<DeliveryMethod>("delivery_method").insert_one(&method).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "delivery_method.created", "delivery_method", &method.uuid,
        Some(method.name.clone()),
    ).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(method))
}

/// Shared with `update`, so a price or a window can't be made nonsensical from
/// either end. `place_order` has no floor of its own — a negative price here
/// becomes a negative shipping line on the order.
pub fn validate(body: &ReqBody) -> Result<(), String> {
    if body.name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    if body.price < 0 {
        return Err("Price can't be negative".to_string());
    }
    if let Some(free_above) = body.free_above {
        if free_above < 0 {
            return Err("Free-above threshold can't be negative".to_string());
        }
    }
    if body.min_days > body.max_days {
        return Err("The earliest delivery day can't be after the latest".to_string());
    }
    if let Some(cutoff) = body.cutoff_minutes {
        if cutoff >= 24 * 60 {
            return Err("Cutoff must be a time of day".to_string());
        }
    }
    Ok(())
}
