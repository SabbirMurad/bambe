use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Order::{
    Order, OrderItem, ReturnRequest, ReturnItem, ReturnStatus, PaymentStatus, FulfilmentStatus,
};
use crate::model::Product::{ProductVariant, StockMovement, StockReason};
use crate::model::Payment::{PaymentTransaction, PaymentMethodKind, TransactionKind, TransactionStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    status: ReturnStatus,
    staff_note: Option<String>,
    // Only read when moving to Refunded. Defaults to the value of the returned
    // lines at the prices on the order, and is capped at what the order has
    // left to give back.
    refund_amount: Option<Cents>,
    // Whether the returned units go back on sale. Damaged goods don't.
    #[serde(default = "default_restock")]
    restock: bool,
}

fn default_restock() -> bool { true }

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
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

    // Refunded is terminal: the money moved and the lines were marked. Letting
    // it be set twice would double-refund and double-restock.
    if matches!(request.status, ReturnStatus::Refunded) {
        return Ok(Response::bad_request("This return has already been refunded"));
    }
    if matches!(request.status, ReturnStatus::Rejected) && !matches!(body.status, ReturnStatus::Rejected) {
        return Ok(Response::bad_request("A rejected return can't be reopened"));
    }

    let now = Utc::now().timestamp_millis();
    let refunding = matches!(body.status, ReturnStatus::Refunded);

    let mut set_doc = doc! {
        "status": mongodb::bson::to_bson(&body.status).unwrap(),
    };
    if let Some(note) = &body.staff_note {
        set_doc.insert("staff_note", note);
    }
    if matches!(body.status, ReturnStatus::Rejected) || refunding {
        set_doc.insert("resolved_at", now);
        set_doc.insert("resolved_by", &admin.user_id);
    }

    let mut refunded_amount: Option<Cents> = None;
    // Captured here because `order` is scoped to the refund branch below.
    let mut refund_currency = String::new();

    if refunding {
        let order = db
            .collection::<Order>("order")
            .find_one(doc! { "uuid": &request.order_id })
            .await
            .ok()
            .flatten();
        let order = match order {
            Some(o) => o,
            None => return Ok(Response::bad_request("The order behind this return no longer exists")),
        };

        let mut line_cursor = db
            .collection::<ReturnItem>("return_item")
            .find(doc! { "return_request_id": &request_id })
            .await
            .expect("return item lookup");
        let mut lines = Vec::new();
        while let Some(result) = line_cursor.next().await {
            match result {
                Ok(l) => lines.push(l),
                Err(error) => log::error!("skipping unreadable return_item: {:?}", error),
            }
        }
        if lines.is_empty() {
            return Ok(Response::bad_request("This return has no lines to refund"));
        }

        // Value the lines at what was actually paid for them, and mark each
        // order line as refunded. This is the only writer of
        // `refunded_quantity` / `refunded_amount`, which the customer's return
        // form and both order detail endpoints have always read.
        let mut line_value: Cents = 0;
        for line in &lines {
            let order_item = db
                .collection::<OrderItem>("order_item")
                .find_one(doc! { "order_id": &request.order_id, "variant_id": &line.variant_id })
                .await
                .ok()
                .flatten();

            let order_item = match order_item {
                Some(i) => i,
                None => {
                    log::error!(
                        "return {} references variant {} which isn't on order {}",
                        request_id, line.variant_id, request.order_id
                    );
                    continue;
                }
            };

            let remaining = order_item.quantity.saturating_sub(order_item.refunded_quantity);
            let quantity = line.quantity.min(remaining);
            if quantity == 0 {
                continue;
            }

            let value = order_item.unit_price * quantity as i64;
            line_value += value;

            db.collection::<OrderItem>("order_item")
                .update_one(
                    doc! { "uuid": &order_item.uuid },
                    doc! { "$inc": { "refunded_quantity": quantity as i64, "refunded_amount": value } },
                )
                .await
                .ok();

            if body.restock {
                let restored = db
                    .collection::<ProductVariant>("product_variant")
                    .find_one_and_update(
                        doc! { "uuid": &line.variant_id },
                        doc! { "$inc": { "stock": quantity as i32 }, "$set": { "modified_at": now } },
                    )
                    .return_document(ReturnDocument::After)
                    .await
                    .ok()
                    .flatten();

                if let Some(variant) = restored {
                    let movement = StockMovement {
                        uuid: Uuid::now_v7().to_string(),
                        variant_id: line.variant_id.clone(),
                        delta: quantity as i32,
                        balance_after: variant.stock,
                        reason: StockReason::Return,
                        order_id: Some(request.order_id.clone()),
                        note: Some(format!("return {}", request_id)),
                        created_by: Some(admin.user_id.clone()),
                        created_at: now,
                    };
                    db.collection::<StockMovement>("stock_movement").insert_one(&movement).await.ok();

                    db.collection::<crate::model::Product::Product>("product")
                        .update_one(
                            doc! { "uuid": &order_item.product_id },
                            doc! { "$inc": { "sold_count": -(quantity as i64) }, "$set": { "in_stock": true } },
                        )
                        .await
                        .ok();
                }
            }
        }

        // What the order still has left to give back, computed from the
        // transactions rather than the order document — the same arithmetic as
        // a manual refund, so the two paths can't disagree.
        let mut tx_cursor = db
            .collection::<PaymentTransaction>("payment_transaction")
            .find(doc! { "order_id": &request.order_id })
            .await
            .expect("payment transaction lookup");
        let mut transactions = Vec::new();
        while let Some(result) = tx_cursor.next().await {
            let t = match result {
                Ok(t) => t,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            transactions.push(t);
        }
        let already_refunded: Cents = transactions
            .iter()
            .filter(|t| t.kind == TransactionKind::Refund && t.status == TransactionStatus::Succeeded)
            .map(|t| t.amount)
            .sum();
        // What can be given back is what was actually taken, not what the order
        // is worth. A cash-on-delivery order that was never collected has no
        // Sale transaction, so this is 0 and no refund is invented — the lines
        // are still marked returned and the stock still goes back.
        let captured: Cents = transactions
            .iter()
            .filter(|t| t.kind == TransactionKind::Sale && t.status == TransactionStatus::Succeeded)
            .map(|t| t.amount)
            .sum();
        let remaining = (captured - already_refunded).max(0);

        let amount = body.refund_amount.unwrap_or(line_value).clamp(0, remaining);

        // A COD order that was never collected has no money to give back; the
        // lines are still marked and restocked above.
        if amount > 0 {
            let original_sale = transactions.iter().find(|t| t.kind == TransactionKind::Sale);
            let refund = PaymentTransaction {
                uuid: Uuid::now_v7().to_string(),
                order_id: request.order_id.clone(),
                user_id: order.user_id.clone(),
                kind: TransactionKind::Refund,
                status: TransactionStatus::Succeeded,
                amount,
                currency: order.currency.clone(),
                provider: "mock".to_string(),
                provider_transaction_id: Uuid::now_v7().to_string(),
                method_kind: original_sale.map(|t| t.method_kind.clone()).unwrap_or(PaymentMethodKind::Card),
                payment_method_id: original_sale.and_then(|t| t.payment_method_id.clone()),
                failure_code: None,
                failure_message: None,
                created_at: now,
                settled_at: Some(now),
            };
            if let Err(error) = db.collection::<PaymentTransaction>("payment_transaction").insert_one(&refund).await {
                log::error!("{:?}", error);
                return Ok(Response::internal_server_error(&error.to_string()));
            }

            let payment_status = if amount >= remaining { PaymentStatus::Refunded } else { PaymentStatus::PartiallyRefunded };
            let mut order_set = doc! {
                "payment_status": mongodb::bson::to_bson(&payment_status).unwrap(),
                "modified_at": now,
            };

            // Once every unit on the order has come back, the order itself is
            // Returned rather than Delivered.
            if fully_returned(&db, &request.order_id).await {
                order_set.insert("fulfilment_status", mongodb::bson::to_bson(&FulfilmentStatus::Returned).unwrap());
            }

            db.collection::<Order>("order")
                .update_one(doc! { "uuid": &request.order_id }, doc! { "$set": order_set })
                .await
                .ok();
        } else if fully_returned(&db, &request.order_id).await {
            db.collection::<Order>("order")
                .update_one(
                    doc! { "uuid": &request.order_id },
                    doc! { "$set": {
                        "fulfilment_status": mongodb::bson::to_bson(&FulfilmentStatus::Returned).unwrap(),
                        "modified_at": now,
                    } },
                )
                .await
                .ok();
        }

        set_doc.insert("refund_amount", amount);
        refunded_amount = Some(amount);
        refund_currency = order.currency.clone();
    }

    let result = db
        .collection::<ReturnRequest>("return_request")
        .update_one(doc! { "uuid": &request_id }, doc! { "$set": set_doc })
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db,
        &admin.user_id,
        &name,
        "return.resolved",
        "return_request",
        &request_id,
        Some(match refunded_amount {
            Some(amount) => format!("{} refunded {}", body.status, amount),
            None => body.status.to_string(),
        }),
    ).await;

    // The order number is what the customer recognises, so it's worth the
    // extra read rather than quoting an internal return id at them.
    let order_number = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &request.order_id })
        .await
        .ok()
        .flatten()
        .map(|o| o.number)
        .unwrap_or_else(|| "your order".to_string());

    let (kind, title, message, email) = match refunded_amount {
        Some(amount) if amount > 0 => (
            crate::model::Notification::NotifType::RefundIssued,
            format!("Refund issued for order {}", order_number),
            "Your return was accepted and the refund is on its way.".to_string(),
            Some(crate::utils::notify::Email {
                subject: format!("Your Bambe refund for order {}", order_number),
                html: crate::Integrations::Resend::refund_issued_template(&order_number, amount, &refund_currency),
            }),
        ),
        _ => (
            crate::model::Notification::NotifType::OrderConfirmed,
            format!("Return update for order {}", order_number),
            format!("Your return is now {}.", body.status),
            Some(crate::utils::notify::Email {
                subject: format!("An update on your Bambe return for order {}", order_number),
                html: crate::Integrations::Resend::return_update_template(
                    &order_number,
                    &body.status.to_string(),
                    body.staff_note.as_deref(),
                ),
            }),
        ),
    };

    crate::utils::notify::send(
        &db,
        &request.user_id,
        crate::utils::notify::Notice {
            kind,
            title,
            body: message,
            image_id: None,
            related_id: Some(request_id.clone()),
            link: Some("/account/returns".to_string()),
            email,
        },
    ).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "uuid": request_id,
        "status": body.status.to_string(),
        "refund_amount": refunded_amount,
    })))
}

/// True once no line on the order has any unrefunded units left.
async fn fully_returned(db: &mongodb::Database, order_id: &str) -> bool {
    let mut cursor = match db
        .collection::<OrderItem>("order_item")
        .find(doc! { "order_id": order_id })
        .await
    {
        Ok(cursor) => cursor,
        Err(error) => { log::error!("{:?}", error); return false; }
    };

    let mut any = false;
    while let Some(result) = cursor.next().await {
        match result {
            Ok(item) => {
                any = true;
                if item.refunded_quantity < item.quantity {
                    return false;
                }
            }
            Err(error) => {
                log::error!("skipping unreadable order_item: {:?}", error);
                return false;
            }
        }
    }
    any
}
