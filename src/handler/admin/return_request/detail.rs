use std::collections::HashMap;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Order::{Order, OrderItem, ReturnRequest, ReturnItem};
use crate::model::Payment::{PaymentTransaction, TransactionKind, TransactionStatus};

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let request_id = path.into_inner();

    let request = db
        .collection::<ReturnRequest>("return_request")
        .find_one(doc! { "uuid": &request_id })
        .await
        .ok()
        .flatten();
    let request = match request {
        Some(r) => r,
        None => return Ok(Response::not_found("Return request not found")),
    };

    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &request.order_id })
        .await
        .ok()
        .flatten();

    // The order lines carry the price and title; the return lines carry only a
    // variant id and a quantity, so the two are joined here rather than in the
    // browser.
    let mut order_items: HashMap<String, OrderItem> = HashMap::new();
    let mut item_cursor = db
        .collection::<OrderItem>("order_item")
        .find(doc! { "order_id": &request.order_id })
        .await
        .expect("order item lookup");
    while let Some(result) = item_cursor.next().await {
        match result {
            Ok(i) => { order_items.insert(i.variant_id.clone(), i); }
            Err(error) => log::error!("skipping unreadable order_item: {:?}", error),
        }
    }

    let mut line_cursor = db
        .collection::<ReturnItem>("return_item")
        .find(doc! { "return_request_id": &request_id })
        .await
        .expect("return item lookup");

    let mut lines = Vec::new();
    let mut line_value: i64 = 0;
    while let Some(result) = line_cursor.next().await {
        let line = match result {
            Ok(l) => l,
            Err(error) => { log::error!("skipping unreadable return_item: {:?}", error); continue; }
        };
        let order_item = order_items.get(&line.variant_id);
        // Refund at the price actually paid for the line, not the current
        // catalogue price.
        let unit_price = order_item.map(|i| i.unit_price).unwrap_or(0);
        let line_refund = unit_price * line.quantity as i64;
        line_value += line_refund;

        lines.push(json!({
            "uuid": line.uuid,
            "variant_id": line.variant_id,
            "quantity": line.quantity,
            "title": order_item.map(|i| i.title.clone()),
            "variant_label": order_item.and_then(|i| i.variant_label.clone()),
            "sku": order_item.map(|i| i.sku.clone()),
            "image_id": order_item.and_then(|i| i.image_id.clone()),
            "unit_price": unit_price,
            "line_refund": line_refund,
            "ordered_quantity": order_item.map(|i| i.quantity),
            "already_refunded": order_item.map(|i| i.refunded_quantity),
        }));
    }

    /*
      What the lines are worth is not the same as what can be given back.
      `resolve.rs` clamps a refund to what was actually captured less what has
      already been returned, so the same ceiling is computed here — otherwise
      the dialog offers an amount the server then silently reduces, and staff
      believe they refunded money that never moved.
    */
    let mut tx_cursor = db
        .collection::<PaymentTransaction>("payment_transaction")
        .find(doc! { "order_id": &request.order_id })
        .await
        .expect("payment transaction lookup");
    let mut captured: i64 = 0;
    let mut already_refunded: i64 = 0;
    while let Some(result) = tx_cursor.next().await {
        match result {
            Ok(t) if t.status == TransactionStatus::Succeeded => match t.kind {
                TransactionKind::Sale => captured += t.amount,
                TransactionKind::Refund => already_refunded += t.amount,
                _ => {}
            },
            Ok(_) => {}
            Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
        }
    }
    let ceiling = (captured - already_refunded).max(0);

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "request": {
            "uuid": request.uuid,
            "order_id": request.order_id,
            "user_id": request.user_id,
            "reason": request.reason,
            "details": request.details,
            "image_ids": request.image_ids,
            "status": request.status.to_string(),
            "refund_amount": request.refund_amount,
            "staff_note": request.staff_note,
            "requested_at": request.requested_at,
            "resolved_at": request.resolved_at,
            "resolved_by": request.resolved_by,
        },
        "order": order.as_ref().map(|o| json!({
            "uuid": o.uuid,
            "number": o.number,
            "customer": o.shipping_address.recipient_name,
            "currency": o.currency,
            "grand_total": o.totals.grand_total,
            "payment_status": o.payment_status.to_string(),
            "fulfilment_status": o.fulfilment_status.to_string(),
            "placed_at": o.placed_at,
        })),
        "items": lines,
        // What the returned lines are worth at the prices on the order.
        "line_value": line_value,
        // What a refund can actually pay out — the line value capped by what
        // is left of the captured payment. Zero for an uncollected COD order.
        "refundable": line_value.min(ceiling),
        "captured": captured,
        "already_refunded": already_refunded,
    })))
}
