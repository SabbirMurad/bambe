use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::{Order, Tracking, ShipmentEvent};

#[derive(Debug, Deserialize)]
pub struct EventInput {
    status: String,
    description: Option<String>,
    location: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    carrier: String,
    tracking_number: String,
    tracking_url: Option<String>,
    estimated_delivery: Option<i64>,
    // An optional timeline entry to append alongside the tracking details —
    // "Out for delivery", "Delivered to concierge", etc.
    event: Option<EventInput>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let order_id = path.into_inner();

    if db.collection::<Order>("order").find_one(doc! { "uuid": &order_id }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Order not found"));
    }

    let tracking = Tracking {
        carrier: body.carrier.clone(),
        tracking_number: body.tracking_number.clone(),
        tracking_url: body.tracking_url.clone(),
        estimated_delivery: body.estimated_delivery,
    };

    let now = Utc::now().timestamp_millis();
    let result = db
        .collection::<Order>("order")
        .update_one(
            doc! { "uuid": &order_id },
            doc! { "$set": { "tracking": mongodb::bson::to_bson(&tracking).unwrap(), "modified_at": now } },
        )
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    if let Some(event) = &body.event {
        let shipment_event = ShipmentEvent {
            uuid: Uuid::now_v7().to_string(),
            order_id: order_id.clone(),
            status: event.status.clone(),
            description: event.description.clone(),
            location: event.location.clone(),
            at: now,
        };
        if let Err(error) = db.collection::<ShipmentEvent>("shipment_event").insert_one(&shipment_event).await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    Ok(Response::ok_message("Tracking updated"))
}
