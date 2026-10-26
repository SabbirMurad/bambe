use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cents;
use crate::model::Store::{ShippingZone, ShippingRateKind};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: Option<String>,
    countries: Option<Vec<String>>,
    rate_kind: Option<ShippingRateKind>,
    rate: Option<Cents>,
    free_above: Option<Cents>,
    enabled: Option<bool>,
    position: Option<u16>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Shipping).await?;
    let db = MongoDB.connect();
    let col = db.collection::<ShippingZone>("shipping_zone");
    let zone_id = path.into_inner();

    if col.find_one(doc! { "uuid": &zone_id }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Shipping zone not found"));
    }

    let mut set_doc = Document::new();
    if let Some(v) = &body.name { set_doc.insert("name", v); }
    if let Some(v) = &body.countries { set_doc.insert("countries", v); }
    if let Some(v) = &body.rate_kind { set_doc.insert("rate_kind", mongodb::bson::to_bson(v).unwrap()); }
    if let Some(v) = body.rate { set_doc.insert("rate", v); }
    if body.free_above.is_some() { set_doc.insert("free_above", body.free_above); }
    if let Some(v) = body.enabled { set_doc.insert("enabled", v); }
    if let Some(v) = body.position { set_doc.insert("position", v as i32); }
    set_doc.insert("modified_at", Utc::now().timestamp_millis());

    let result = col.update_one(doc! { "uuid": &zone_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let updated = col.find_one(doc! { "uuid": &zone_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
