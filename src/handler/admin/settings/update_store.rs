use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Store::StoreSettings;
use super::STORE_SETTINGS_ID;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    store_name: Option<String>,
    support_email: Option<String>,
    support_phone: Option<String>,
    facebook_url: Option<String>,
    instagram_url: Option<String>,
    business_address: Option<String>,
    default_currency: Option<String>,
    timezone: Option<String>,
    logo_image_id: Option<String>,
    order_number_prefix: Option<String>,
    ticket_number_prefix: Option<String>,
    tax_rate_bps: Option<u32>,
    prices_include_tax: Option<bool>,
    maintenance_mode: Option<bool>,
    theme: Option<String>,
}

/// The themes the storefront's stylesheet defines.
pub const THEMES: [&str; 3] = ["orange", "crimson", "green"];

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<StoreSettings>("store_settings");

    // $set and $setOnInsert can't both touch the same field on an upsert
    // insert, so every default below is only added when the caller didn't
    // already supply that field in $set.
    if let Some(theme) = &body.theme {
        if !THEMES.contains(&theme.as_str()) {
            return Ok(Response::bad_request("Unknown theme"));
        }
    }

    let mut set_doc = Document::new();
    let mut on_insert = doc! { "uuid": STORE_SETTINGS_ID };

    macro_rules! field {
        ($name:literal, $value:expr, $default:expr) => {
            match $value {
                Some(v) => { set_doc.insert($name, v); }
                None => { on_insert.insert($name, $default); }
            }
        };
    }

    field!("store_name", &body.store_name, "My Store");
    field!("support_email", &body.support_email, "");
    field!("business_address", &body.business_address, "");
    field!("default_currency", &body.default_currency, "USD");
    field!("timezone", &body.timezone, "UTC");
    field!("order_number_prefix", &body.order_number_prefix, "OR");
    field!("ticket_number_prefix", &body.ticket_number_prefix, "TK");
    field!("tax_rate_bps", body.tax_rate_bps, 0u32);
    field!("prices_include_tax", body.prices_include_tax, false);
    field!("maintenance_mode", body.maintenance_mode, false);

    // Optional fields need no insert-time default — a missing key
    // deserializes to `None` on `Option<T>`.
    // An empty string clears these; leaving the key out keeps them.
    if let Some(v) = &body.support_phone { set_doc.insert("support_phone", v.trim()); }
    for (key, value) in [("facebook_url", &body.facebook_url), ("instagram_url", &body.instagram_url)] {
        if let Some(v) = value {
            let v = v.trim();
            if !v.is_empty() && !(v.starts_with("https://") || v.starts_with("http://")) {
                return Ok(Response::bad_request("Social links must be full addresses starting with https://"));
            }
            set_doc.insert(key, v);
        }
    }
    if let Some(v) = &body.logo_image_id { set_doc.insert("logo_image_id", v); }
    if let Some(v) = &body.theme { set_doc.insert("theme", v); }

    set_doc.insert("modified_at", Utc::now().timestamp_millis());
    set_doc.insert("modified_by", &admin.user_id);

    let result = col
        .update_one(
            doc! { "uuid": STORE_SETTINGS_ID },
            doc! { "$set": set_doc, "$setOnInsert": on_insert },
        )
        .upsert(true)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let updated = col.find_one(doc! { "uuid": STORE_SETTINGS_ID }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
