use chrono::Utc;
use mongodb::bson::doc;
use uuid::Uuid;
use serde::Deserialize;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Customer;

// Every field optional: the settings screen sends only the switch that moved,
// so a partial body must not reset the rest.
#[derive(Debug, Deserialize)]
pub struct ReqBody {
    order_updates: Option<bool>,
    delivery_updates: Option<bool>,
    support_replies: Option<bool>,
    security_alerts: Option<bool>,
    price_drops: Option<bool>,
    back_in_stock: Option<bool>,
    review_reminders: Option<bool>,
    promotions: Option<bool>,
    weekly_digest: Option<bool>,
    push_enabled: Option<bool>,
    email_enabled: Option<bool>,
    sms_enabled: Option<bool>,
}

pub async fn task(
    req: HttpRequest,
    body: web::Json<ReqBody>,
) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let user_id = user.user_id;
    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();
    let col = db.collection::<Customer::NotificationPreferences>("notification_preferences");

    let existing = col.find_one(doc!{ "user_id": &user_id }).await.ok().flatten();

    let mut set_doc = mongodb::bson::Document::new();
    set_doc.insert("modified_at", now);

    /*
      Write a field when the body carries it, and otherwise only when the row
      is being created — so the first save lands a complete document instead of
      one holding whichever switch happened to be touched first.
    */
    macro_rules! apply_field {
        ($field:ident) => {
            if let Some(v) = body.$field {
                set_doc.insert(stringify!($field), v);
            } else if existing.is_none() {
                set_doc.insert(stringify!($field), true);
            }
        };
        ($field:ident, $default:expr) => {
            if let Some(v) = body.$field {
                set_doc.insert(stringify!($field), v);
            } else if existing.is_none() {
                set_doc.insert(stringify!($field), $default);
            }
        };
    }

    apply_field!(order_updates);
    apply_field!(delivery_updates);
    apply_field!(support_replies);
    apply_field!(security_alerts);
    apply_field!(price_drops);
    apply_field!(back_in_stock);
    apply_field!(review_reminders);

    // Marketing is opt-in, so it defaults off rather than on.
    apply_field!(promotions, false);
    apply_field!(weekly_digest, false);

    apply_field!(push_enabled);
    apply_field!(email_enabled);
    // SMS costs money per message and few shops want it on by default.
    apply_field!(sms_enabled, false);

    let result = col.update_one(
        doc!{ "user_id": &user_id },
        doc!{
            "$set": set_doc,
            "$setOnInsert": { "uuid": Uuid::now_v7().to_string(), "user_id": &user_id }
        },
    )
    .upsert(true)
    .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("Preferences updated"))
}
