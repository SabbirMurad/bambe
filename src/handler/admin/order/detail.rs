use std::collections::HashMap;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Account::{AccountCore, AccountProfile};
use crate::model::Order::{Order, OrderItem, ShipmentEvent};
use crate::model::Payment::{PaymentTransaction, TransactionKind, TransactionStatus};
use crate::model::ImageStruct;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let order_id = path.into_inner();

    let order = db.collection::<Order>("order").find_one(doc! { "uuid": &order_id }).await.ok().flatten();
    let order = match order {
        Some(o) => o,
        None => return Ok(Response::not_found("Order not found")),
    };

    let order_total = order.totals.grand_total;

    let customer = db.collection::<AccountCore>("account_core").find_one(doc! { "uuid": &order.user_id }).await.ok().flatten();
    // The name the customer gave, not the generated username a Google
    // sign-in leaves behind ("user_0193a1f2").
    let profile = db.collection::<AccountProfile>("account_profile").find_one(doc! { "uuid": &order.user_id }).await.ok().flatten();

    let mut item_cursor = db.collection::<OrderItem>("order_item").find(doc! { "order_id": &order_id }).await.expect("order item lookup");
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
                "unit_price": i.unit_price,
                "quantity": i.quantity,
                "line_total": i.line_total,
                "refunded_quantity": i.refunded_quantity,
                "refunded_amount": i.refunded_amount,
            })
        })
        .collect();

    let mut events_cursor = db.collection::<ShipmentEvent>("shipment_event").find(doc! { "order_id": &order_id }).sort(doc! { "at": 1 }).await.expect("shipment event lookup");
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

    /*
      How much of this order has already gone back.

      `refund.rs` works this out from payment_transaction before it will
      accept anything, and answers 400 when there is nothing left — but
      the console had no way to know that in advance, so it could only
      offer a refund box and let the operator find out by clicking.
    */
    let mut tx_cursor = db
        .collection::<PaymentTransaction>("payment_transaction")
        .find(doc! { "order_id": &order_id })
        .await
        .expect("payment transaction lookup");
    let mut refunded_total: i64 = 0;
    while let Some(result) = tx_cursor.next().await {
        let t = match result {
            Ok(t) => t,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        if t.kind == TransactionKind::Refund && t.status == TransactionStatus::Succeeded {
            refunded_total += t.amount;
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "order": order,
        "customer": customer.map(|c| json!({
            "user_id": c.uuid,
            "email": c.email_address,
            "username": c.username,
            "name": profile.as_ref()
                .map(|p| format!("{} {}", p.first_name, p.last_name).trim().to_string())
                .filter(|n| !n.is_empty()),
            "phone": profile.as_ref().and_then(|p| p.phone_number.clone()),
        })),
        "items": item_views,
        "shipment_events": events,
        "refunded_total": refunded_total,
        "refundable": (order_total - refunded_total).max(0),
    })))
}
