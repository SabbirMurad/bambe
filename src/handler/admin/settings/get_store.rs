use chrono::Utc;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Store::StoreSettings;
use super::STORE_SETTINGS_ID;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let col = db.collection::<StoreSettings>("store_settings");

    // `super::load` also adopts a row stored under some other uuid, so the
    // console cannot end up editing a second document beside the one
    // checkout reads.
    if let Some(settings) = super::load(&db).await {
        return Ok(HttpResponse::Ok().content_type("application/json").json(settings));
    }

    let defaults = StoreSettings {
        uuid: STORE_SETTINGS_ID.to_string(),
        store_name: "My Store".to_string(),
        support_email: String::new(),
        support_phone: None,
        facebook_url: None,
        instagram_url: None,
        business_address: String::new(),
        default_currency: "BDT".to_string(),
        timezone: "UTC".to_string(),
        logo_image_id: None,
        order_number_prefix: "OR".to_string(),
        ticket_number_prefix: "TK".to_string(),
        tax_rate_bps: 0,
        prices_include_tax: false,
        maintenance_mode: false,
        theme: None,
        modified_at: Utc::now().timestamp_millis(),
        modified_by: None,
    };

    if let Err(error) = col.insert_one(&defaults).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(defaults))
}
