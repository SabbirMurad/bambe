use serde_json::json;
use mongodb::bson::doc;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Customer;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;
    let db = MongoDB.connect();

    let result = db.collection::<Customer::NotificationPreferences>("notification_preferences")
        .find_one(doc!{ "user_id": &user_id }).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    match result.unwrap() {
        Some(p) => Ok(HttpResponse::Ok().content_type("application/json").json(json!({
            "order_updates": p.order_updates,
            "delivery_updates": p.delivery_updates,
            "support_replies": p.support_replies,
            "security_alerts": p.security_alerts,
            "price_drops": p.price_drops,
            "back_in_stock": p.back_in_stock,
            "review_reminders": p.review_reminders,
            "promotions": p.promotions,
            "weekly_digest": p.weekly_digest,
            "push_enabled": p.push_enabled,
            "email_enabled": p.email_enabled,
            "sms_enabled": p.sms_enabled,
        }))),
        /*
          No row yet — most users never open this screen. Return the defaults
          the sender applies rather than 404, so the settings page has
          something to draw and the two agree on what "unset" means.
        */
        None => Ok(HttpResponse::Ok().content_type("application/json").json(json!({
            "order_updates": true,
            "delivery_updates": true,
            "support_replies": true,
            "security_alerts": true,
            "price_drops": true,
            "back_in_stock": true,
            "review_reminders": true,
            "promotions": false,
            "weekly_digest": false,
            "push_enabled": true,
            "email_enabled": true,
            "sms_enabled": false,
        }))),
    }
}
