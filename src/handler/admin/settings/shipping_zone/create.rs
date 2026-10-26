use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cents;
use crate::model::Store::{ShippingZone, ShippingRateKind};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: String,
    #[serde(default)]
    countries: Vec<String>,
    rate_kind: ShippingRateKind,
    rate: Cents,
    free_above: Option<Cents>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    position: u16,
}
fn default_true() -> bool { true }

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Shipping).await?;
    let db = MongoDB.connect();

    if body.name.trim().is_empty() {
        return Ok(Response::bad_request("Name is required"));
    }

    let zone = ShippingZone {
        uuid: Uuid::now_v7().to_string(),
        name: body.name.clone(),
        countries: body.countries.clone(),
        rate_kind: body.rate_kind.clone(),
        rate: body.rate,
        free_above: body.free_above,
        enabled: body.enabled,
        position: body.position,
        modified_at: Utc::now().timestamp_millis(),
    };

    if let Err(error) = db.collection::<ShippingZone>("shipping_zone").insert_one(&zone).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(zone))
}
