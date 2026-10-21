//! Undoing what an order held.
//!
//! Placing an order takes two things that have to be given back if it never
//! completes: the stock it reserved, and the coupon redemption it burned. Both
//! the customer's own cancel, the console's cancel, and an abandoned or
//! declined gateway payment need exactly the same undo, so it lives here
//! rather than being written out three times and drifting.
//!
//! Best-effort and idempotent-ish: the stock `$inc` is guarded by nothing, so
//! callers must only invoke this once per order — every caller does it behind
//! a conditional status update that claims the order first.

use futures::StreamExt;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use uuid::Uuid;
use crate::model::Order::OrderItem;
use crate::model::Product::{Product, ProductVariant, StockMovement, StockReason};
use crate::model::Coupon::{Coupon, CouponRedemption};

/// Put back the stock and the coupon an order was holding.
pub async fn release_order_holds(db: &mongodb::Database, order_id: &str, now: i64) {
    let mut cursor = match db
        .collection::<OrderItem>("order_item")
        .find(doc! { "order_id": order_id })
        .await
    {
        Ok(cursor) => cursor,
        Err(error) => {
            log::error!("could not read lines of {} to release stock: {:?}", order_id, error);
            return;
        }
    };

    let mut items = Vec::new();
    while let Some(result) = cursor.next().await {
        match result {
            Ok(item) => items.push(item),
            Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
        }
    }

    for item in &items {
        // Atomic `$inc`, and the post-update document gives the movement log
        // the true balance rather than one computed from an earlier read.
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
                order_id: Some(order_id.to_string()),
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

    release_coupon(db, order_id).await;
}

/// Drop the redemption and decrement the coupon's use count, so an order that
/// never happened stops burning the customer's per-user limit.
pub async fn release_coupon(db: &mongodb::Database, order_id: &str) {
    let redemption = db
        .collection::<CouponRedemption>("coupon_redemption")
        .find_one_and_delete(doc! { "order_id": order_id })
        .await
        .ok()
        .flatten();

    if let Some(redemption) = redemption {
        db.collection::<Coupon>("coupon")
            .update_one(
                // Guarded so a double release can't drive the count below zero.
                doc! { "uuid": &redemption.coupon_id, "used_count": { "$gt": 0 } },
                doc! { "$inc": { "used_count": -1 } },
            )
            .await
            .ok();
    }
}
