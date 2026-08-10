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
use crate::model::Cents;
use crate::model::Order::{Order, PaymentStatus};
use crate::model::Payment::{PaymentTransaction, PaymentMethodKind, TransactionKind, TransactionStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    // Defaults to whatever's left unrefunded on the order.
    amount: Option<Cents>,
    #[allow(dead_code)]
    reason: Option<String>,
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

    if !matches!(order.payment_status, PaymentStatus::Paid | PaymentStatus::PartiallyRefunded) {
        return Ok(Response::bad_request("This order has nothing left to refund"));
    }

    let mut tx_cursor = db
        .collection::<PaymentTransaction>("payment_transaction")
        .find(doc! { "order_id": &order_id })
        .await
        .expect("payment transaction lookup");
    let mut transactions = Vec::new();
    while let Some(Ok(t)) = tx_cursor.next().await {
        transactions.push(t);
    }

    let already_refunded: i64 = transactions
        .iter()
        .filter(|t| t.kind == TransactionKind::Refund && t.status == TransactionStatus::Succeeded)
        .map(|t| t.amount)
        .sum();
    let remaining = order.totals.grand_total - already_refunded;
    if remaining <= 0 {
        return Ok(Response::bad_request("This order has already been fully refunded"));
    }

    let amount = body.amount.unwrap_or(remaining).min(remaining);
    if amount <= 0 {
        return Ok(Response::bad_request("Refund amount must be positive"));
    }

    let original_sale = transactions.iter().find(|t| t.kind == TransactionKind::Sale);
    let now = Utc::now().timestamp_millis();

    let refund = PaymentTransaction {
        uuid: Uuid::now_v7().to_string(),
        order_id: order_id.clone(),
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

    let new_payment_status = if amount >= remaining { PaymentStatus::Refunded } else { PaymentStatus::PartiallyRefunded };
    let result = db
        .collection::<Order>("order")
        .update_one(
            doc! { "uuid": &order_id },
            doc! {
                "$set": {
                    "payment_status": mongodb::bson::to_bson(&new_payment_status).unwrap(),
                    "modified_at": now,
                },
            },
        )
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(&db, &admin.user_id, &name, "order.refunded", "order", &order_id, Some(format!("{} {}", amount, order.currency))).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(refund))
}
