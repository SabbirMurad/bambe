use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Order::{Order, OrderItem, FulfilmentStatus};
use crate::model::Product::{Product, ProductVariant, StockMovement, StockReason};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    fulfilment_status: FulfilmentStatus,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Fulfilment).await?;
    let db = MongoDB.connect();
    let order_id = path.into_inner();

    let order = db.collection::<Order>("order").find_one(doc! { "uuid": &order_id }).await.ok().flatten();
    let order = match order {
        Some(o) => o,
        None => return Ok(Response::not_found("Order not found")),
    };

    let now = Utc::now().timestamp_millis();
    let mut set_doc = doc! { "modified_at": now };
    set_doc.insert("fulfilment_status", mongodb::bson::to_bson(&body.fulfilment_status).unwrap());

    match body.fulfilment_status {
        FulfilmentStatus::Shipped if order.shipped_at.is_none() => { set_doc.insert("shipped_at", now); }
        FulfilmentStatus::Delivered if order.delivered_at.is_none() => { set_doc.insert("delivered_at", now); }
        _ => {}
    }

    // Cancelling from the console restocks exactly like a customer
    // cancellation does — it's the same event, just triggered by staff.
    let newly_cancelled = body.fulfilment_status == FulfilmentStatus::Cancelled
        && order.fulfilment_status != FulfilmentStatus::Cancelled;

    if newly_cancelled {
        set_doc.insert("cancelled_at", now);

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

        for item in &items {
            // Atomic `$inc`, not read-then-`$set`: see `order/cancel.rs`.
            let restored = db
                .collection::<ProductVariant>("product_variant")
                .find_one_and_update(
                    doc! { "uuid": &item.variant_id },
                    doc! { "$inc": { "stock": item.quantity as i32 }, "$set": { "modified_at": now } },
                )
                .return_document(ReturnDocument::After)
                .await
                .ok()
                .flatten();

            if let Some(variant) = restored {
                let movement = StockMovement {
                    uuid: Uuid::now_v7().to_string(),
                    variant_id: item.variant_id.clone(),
                    delta: item.quantity as i32,
                    balance_after: variant.stock,
                    reason: StockReason::Cancellation,
                    order_id: Some(order_id.clone()),
                    note: None,
                    created_by: None,
                    created_at: now,
                };
                db.collection::<StockMovement>("stock_movement").insert_one(&movement).await.ok();
            }

            db.collection::<Product>("product")
                .update_one(
                    doc! { "uuid": &item.product_id },
                    doc! { "$inc": { "sold_count": -(item.quantity as i64) }, "$set": { "in_stock": true } },
                )
                .await
                .ok();
        }
    }

    let result = db.collection::<Order>("order").update_one(doc! { "uuid": &order_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "order.status_changed", "order", &order_id,
        Some(format!("{:?} -> {:?}", order.fulfilment_status, body.fulfilment_status)),
    ).await;

    if newly_cancelled {
        crate::util::order::release_coupon(&db, &order_id).await;
    }

    // Tell the customer, but only about the transitions they'd care about and
    // only when the status actually moved — re-saving "Shipped" on an already
    // shipped order shouldn't send a second dispatch notice.
    if body.fulfilment_status != order.fulfilment_status {
        use crate::model::Notification::NotifType;
        use crate::util::notify::{send, Email, Notice};

        let notice = match body.fulfilment_status {
            FulfilmentStatus::Shipped => Some((
                NotifType::OrderShipped,
                format!("Order {} has shipped", order.number),
                "It's with the courier now.".to_string(),
                Some(Email {
                    subject: format!("Your Bambee order {} is on its way", order.number),
                    html: crate::Integrations::Resend::order_shipped_template(
                        &order.number,
                        order.tracking.as_ref().map(|t| t.tracking_number.as_str()),
                        &order.delivery.name,
                    ),
                }),
            )),
            FulfilmentStatus::Delivered => Some((
                NotifType::OrderDelivered,
                format!("Order {} was delivered", order.number),
                "Let us know how it went.".to_string(),
                Some(Email {
                    subject: format!("Your Bambee order {} has arrived", order.number),
                    html: crate::Integrations::Resend::order_delivered_template(&order.number),
                }),
            )),
            FulfilmentStatus::Cancelled => Some((
                NotifType::OrderCancelled,
                format!("Order {} was cancelled", order.number),
                "It won't be shipped.".to_string(),
                Some(Email {
                    subject: format!("Your Bambee order {} was cancelled", order.number),
                    html: crate::Integrations::Resend::order_cancelled_template(&order.number, None),
                }),
            )),
            // Pending, Packing and OnHold are internal states; a shopper doesn't
            // need a message for every step of the warehouse.
            _ => None,
        };

        if let Some((kind, title, body_text, email)) = notice {
            send(&db, &order.user_id, Notice {
                kind,
                title,
                body: body_text,
                image_id: None,
                related_id: Some(order_id.clone()),
                link: Some("/account/orders".to_string()),
                email,
            }).await;
        }
    }

    // First time it reaches Delivered: one stamp on the customer's card.
    if body.fulfilment_status == FulfilmentStatus::Delivered && order.fulfilment_status != FulfilmentStatus::Delivered {
        crate::util::loyalty::stamp_delivery(&db, &order.user_id, &order_id, &order.number).await;
    }

    Ok(Response::ok_message("Order status updated"))
}
