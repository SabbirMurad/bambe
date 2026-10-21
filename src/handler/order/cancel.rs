use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Order::{Order, OrderItem, FulfilmentStatus, PaymentStatus};
use crate::model::Product::{Product, ProductVariant, StockMovement, StockReason};
use crate::model::Payment::{PaymentTransaction, TransactionKind, TransactionStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    reason: Option<String>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
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

    // Once it's left the building, cancellation turns into a return.
    if !matches!(order.fulfilment_status, FulfilmentStatus::Pending | FulfilmentStatus::Packing) {
        return Ok(Response::bad_request("This order can no longer be cancelled — request a return instead"));
    }

    let now = Utc::now().timestamp_millis();

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

    for item in &items {
        // One atomic `$inc` rather than read-then-`$set`: a concurrent checkout
        // or admin adjustment between the two would otherwise be overwritten
        // with a total computed before it happened. The returned document is
        // the post-increment one, so the movement logs the true balance.
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

    let payment_status = if order.payment_status == PaymentStatus::Paid {
        let original_sale = db
            .collection::<PaymentTransaction>("payment_transaction")
            .find_one(doc! { "order_id": &order_id, "kind": "Sale" })
            .await
            .ok()
            .flatten();

        let refund = PaymentTransaction {
            uuid: Uuid::now_v7().to_string(),
            order_id: order_id.clone(),
            user_id: user.user_id.clone(),
            kind: TransactionKind::Refund,
            status: TransactionStatus::Succeeded,
            amount: order.totals.grand_total,
            currency: order.currency.clone(),
            provider: "mock".to_string(),
            provider_transaction_id: Uuid::now_v7().to_string(),
            method_kind: original_sale.as_ref().map(|t| t.method_kind.clone()).unwrap_or(crate::model::Payment::PaymentMethodKind::Card),
            payment_method_id: original_sale.and_then(|t| t.payment_method_id),
            failure_code: None,
            failure_message: None,
            created_at: now,
            settled_at: Some(now),
        };
        db.collection::<PaymentTransaction>("payment_transaction").insert_one(&refund).await.ok();
        PaymentStatus::Refunded
    } else {
        order.payment_status.clone()
    };

    let result = db
        .collection::<Order>("order")
        .update_one(
            doc! { "uuid": &order_id },
            doc! {
                "$set": {
                    "fulfilment_status": "Cancelled",
                    "payment_status": mongodb::bson::to_bson(&payment_status).unwrap(),
                    "cancelled_at": now,
                    "cancel_reason": body.reason.clone(),
                    "modified_at": now,
                }
            },
        )
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    crate::util::order::release_coupon(&db, &order_id).await;

    // A confirmation of what they just did, and the record of the refund. The
    // customer triggered this, so it's a receipt rather than news.
    let refunded = if payment_status == PaymentStatus::Refunded {
        Some((order.totals.grand_total, order.currency.as_str()))
    } else {
        None
    };
    crate::util::notify::send(
        &db,
        &user.user_id,
        crate::util::notify::Notice {
            kind: crate::model::Notification::NotifType::OrderCancelled,
            title: format!("Order {} cancelled", order.number),
            body: match refunded {
                Some(_) => "Your refund is on its way.".to_string(),
                None => "Nothing was charged.".to_string(),
            },
            image_id: None,
            related_id: Some(order_id.clone()),
            link: Some("/account/orders".to_string()),
            email: Some(crate::util::notify::Email {
                subject: format!("Your Bambee order {} was cancelled", order.number),
                html: crate::Integrations::Resend::order_cancelled_template(&order.number, refunded),
            }),
        },
    ).await;

    Ok(Response::ok_message("Order cancelled"))
}
