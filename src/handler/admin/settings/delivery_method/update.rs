use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cents;
use crate::model::Order::DeliveryMethod;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: Option<String>,
    description: Option<String>,
    price: Option<Cents>,
    free_above: Option<Cents>,
    min_days: Option<u8>,
    max_days: Option<u8>,
    cutoff_minutes: Option<u16>,
    enabled: Option<bool>,
    position: Option<u16>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Shipping).await?;
    let db = MongoDB.connect();
    let col = db.collection::<DeliveryMethod>("delivery_method");
    let id = path.into_inner();

    let existing = col.find_one(doc! { "uuid": &id }).await.ok().flatten();
    let existing = match existing {
        Some(m) => m,
        None => return Ok(Response::not_found("Delivery method not found")),
    };

    // Check the values the row will end up with, not just the ones supplied —
    // raising `min_days` alone can invert a window that was fine before.
    let price = body.price.unwrap_or(existing.price);
    let free_above = body.free_above.or(existing.free_above);
    let min_days = body.min_days.unwrap_or(existing.min_days);
    let max_days = body.max_days.unwrap_or(existing.max_days);
    let cutoff = body.cutoff_minutes.or(existing.cutoff_minutes);

    if price < 0 {
        return Ok(Response::bad_request("Price can't be negative"));
    }
    if free_above.map(|v| v < 0).unwrap_or(false) {
        return Ok(Response::bad_request("Free-above threshold can't be negative"));
    }
    if min_days > max_days {
        return Ok(Response::bad_request("The earliest delivery day can't be after the latest"));
    }
    if cutoff.map(|c| c >= 24 * 60).unwrap_or(false) {
        return Ok(Response::bad_request("Cutoff must be a time of day"));
    }
    if body.name.as_ref().map(|n| n.trim().is_empty()).unwrap_or(false) {
        return Ok(Response::bad_request("Name is required"));
    }

    // Turning off the last enabled method leaves checkout with nothing to
    // quote, exactly as deleting it would.
    if body.enabled == Some(false) && existing.enabled {
        let others = col
            .count_documents(doc! { "enabled": true, "uuid": { "$ne": &id } })
            .await
            .unwrap_or(0);
        if others == 0 {
            return Ok(Response::bad_request(
                "This is the only delivery method customers can choose — enable another before turning it off",
            ));
        }
    }

    let mut set_doc = Document::new();
    if let Some(v) = &body.name { set_doc.insert("name", v.trim()); }
    if let Some(v) = &body.description { set_doc.insert("description", v.trim()); }
    if let Some(v) = body.price { set_doc.insert("price", v); }
    if body.free_above.is_some() { set_doc.insert("free_above", body.free_above); }
    if let Some(v) = body.min_days { set_doc.insert("min_days", v as i32); }
    if let Some(v) = body.max_days { set_doc.insert("max_days", v as i32); }
    if body.cutoff_minutes.is_some() { set_doc.insert("cutoff_minutes", body.cutoff_minutes.map(|c| c as i32)); }
    if let Some(v) = body.enabled { set_doc.insert("enabled", v); }
    if let Some(v) = body.position { set_doc.insert("position", v as i32); }

    if set_doc.is_empty() {
        return Ok(HttpResponse::Ok().content_type("application/json").json(existing));
    }

    if let Err(error) = col.update_one(doc! { "uuid": &id }, doc! { "$set": set_doc }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "delivery_method.updated", "delivery_method", &id, None,
    ).await;

    let updated = col.find_one(doc! { "uuid": &id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
