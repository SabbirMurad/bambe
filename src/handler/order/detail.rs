use std::collections::HashMap;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::require_shopper;
use crate::model::Order::{Order, OrderItem, ShipmentEvent};
use crate::model::ImageStruct;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_shopper(&req)?;
    let db = MongoDB.connect();
    let order_id = path.into_inner();

    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &order_id, "user_id": &user.user_id })
        .await
        .ok()
        .flatten();
    let order = match order {
        Some(o) => o,
        None => return Ok(Response::not_found("Order not found")),
    };

    let mut item_cursor = db
        .collection::<OrderItem>("order_item")
        .find(doc! { "order_id": &order_id })
        .await
        .expect("order item lookup");
    let mut items = Vec::new();
    while let Some(result) = item_cursor.next().await {
        let i = match result {
            Ok(i) => i,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        items.push(i);
    }

    let image_ids: Vec<String> = items.iter().filter_map(|i| i.image_id.clone()).collect();
    let images: HashMap<String, ImageStruct> = if image_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db.collection::<ImageStruct>("image").find(doc! { "uuid": { "$in": &image_ids } }).await.expect("image lookup");
        let mut map = HashMap::new();
        while let Some(result) = cursor.next().await {
            let i = match result {
                Ok(i) => i,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            map.insert(i.uuid.clone(), i);
        }
        map
    };

    let item_views: Vec<_> = items
        .iter()
        .map(|i| {
            json!({
                "uuid": i.uuid,
                "product_id": i.product_id,
                "variant_id": i.variant_id,
                "title": i.title,
                "variant_label": i.variant_label,
                "sku": i.sku,
                "image": i.image_id.as_ref().and_then(|id| images.get(id)),
                "brand_name": i.brand_name,
                "unit_price": i.unit_price,
                "quantity": i.quantity,
                "line_total": i.line_total,
                "refunded_quantity": i.refunded_quantity,
                "refunded_amount": i.refunded_amount,
            })
        })
        .collect();

    let mut events_cursor = db
        .collection::<ShipmentEvent>("shipment_event")
        .find(doc! { "order_id": &order_id })
        .sort(doc! { "at": 1 })
        .await
        .expect("shipment event lookup");
    let mut events = Vec::new();
    while let Some(result) = events_cursor.next().await {
        let e = match result {
            Ok(e) => e,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        events.push(e);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "uuid": order.uuid,
        "number": order.number,
        "items": item_views,
        "shipping_address": order.shipping_address,
        "billing_address": order.billing_address,
        "delivery": order.delivery,
        "packaging": order.packaging,
        "totals": order.totals,
        "currency": order.currency,
        "fulfilment_status": order.fulfilment_status,
        "payment_status": order.payment_status,
        "payment_summary": order.payment_summary,
        "coupon": order.coupon,
        "customer_note": order.customer_note,
        "tracking": order.tracking,
        "shipment_events": events,
        "placed_at": order.placed_at,
        "paid_at": order.paid_at,
        "shipped_at": order.shipped_at,
        "delivered_at": order.delivered_at,
        "cancelled_at": order.cancelled_at,
        "cancel_reason": order.cancel_reason,
    })))
}
