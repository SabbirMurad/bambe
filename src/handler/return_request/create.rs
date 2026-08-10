use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Order::{Order, OrderItem, ReturnRequest, ReturnItem, ReturnStatus, FulfilmentStatus};

#[derive(Debug, Deserialize)]
pub struct ReturnLine {
    variant_id: String,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    order_id: String,
    items: Vec<ReturnLine>,
    reason: String,
    details: Option<String>,
    #[serde(default)]
    image_ids: Vec<String>,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    if body.items.is_empty() {
        return Ok(Response::bad_request("Select at least one item to return"));
    }

    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &body.order_id, "user_id": &user.user_id })
        .await
        .ok()
        .flatten();
    let order = match order {
        Some(o) => o,
        None => return Ok(Response::not_found("Order not found")),
    };

    if order.fulfilment_status != FulfilmentStatus::Delivered {
        return Ok(Response::bad_request("Only a delivered order can be returned"));
    }

    // Each requested line has to be a real line on this order, and not ask
    // for more than what hasn't already been returned.
    for line in &body.items {
        let order_item = db
            .collection::<OrderItem>("order_item")
            .find_one(doc! { "order_id": &body.order_id, "variant_id": &line.variant_id })
            .await
            .ok()
            .flatten();

        match order_item {
            Some(oi) if line.quantity > 0 && line.quantity <= (oi.quantity - oi.refunded_quantity) => {}
            _ => return Ok(Response::bad_request("One of the selected items can't be returned in that quantity")),
        }
    }

    let now = Utc::now().timestamp_millis();
    let return_request = ReturnRequest {
        uuid: Uuid::now_v7().to_string(),
        order_id: body.order_id.clone(),
        user_id: user.user_id,
        reason: body.reason.clone(),
        details: body.details.clone(),
        image_ids: body.image_ids.clone(),
        status: ReturnStatus::Requested,
        refund_amount: None,
        staff_note: None,
        requested_at: now,
        resolved_at: None,
        resolved_by: None,
    };

    if let Err(error) = db.collection::<ReturnRequest>("return_request").insert_one(&return_request).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let return_items: Vec<ReturnItem> = body
        .items
        .iter()
        .map(|line| ReturnItem {
            uuid: Uuid::now_v7().to_string(),
            return_request_id: return_request.uuid.clone(),
            variant_id: line.variant_id.clone(),
            quantity: line.quantity,
        })
        .collect();

    if let Err(error) = db.collection::<ReturnItem>("return_item").insert_many(&return_items).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(return_request))
}
