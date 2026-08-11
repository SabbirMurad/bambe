use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::{Order, OrderItem, FulfilmentStatus};
use crate::model::Product::{Product, ProductVariant, StockMovement, StockReason};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    fulfilment_status: FulfilmentStatus,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
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
        while let Some(Ok(i)) = item_cursor.next().await {
            items.push(i);
        }

        for item in &items {
            if let Some(variant) = db.collection::<ProductVariant>("product_variant").find_one(doc! { "uuid": &item.variant_id }).await.ok().flatten() {
                let new_stock = variant.stock + item.quantity as i32;
                db.collection::<ProductVariant>("product_variant")
                    .update_one(doc! { "uuid": &item.variant_id }, doc! { "$set": { "stock": new_stock, "modified_at": now } })
                    .await
                    .ok();

                let movement = StockMovement {
                    uuid: Uuid::now_v7().to_string(),
                    variant_id: item.variant_id.clone(),
                    delta: item.quantity as i32,
                    balance_after: new_stock,
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

    Ok(Response::ok_message("Order status updated"))
}
