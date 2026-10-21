//! SSLCommerz callbacks.
//!
//! All four endpoints are **unauthenticated** — SSLCommerz calls them, not a
//! signed-in browser — so nothing in the request body is believed. The only
//! field taken from it is `val_id`, which is handed to the validation API over
//! a server-to-server call authenticated with the store password; the amount,
//! currency and status that come back from *that* are what decide whether an
//! order is paid.
//!
//! Without that, anyone could POST `status=VALID` at `/payment/sslcommerz/ipn`
//! with someone else's transaction id and mark their order paid for free.
//!
//! `success`, `fail` and `cancel` are browser form-POSTs and answer with a
//! redirect to a page. `ipn` is server-to-server and answers with plain text.
//! They share `settle`, because SSLCommerz makes no promise about which
//! arrives first, and a shopper who closes the tab mid-redirect must still end
//! up with a paid order.

use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::model::Cents;
use crate::model::Order::{Order, PaymentStatus, FulfilmentStatus};
use crate::model::Payment::{
    PaymentTransaction, PaymentMethodKind, TransactionKind, TransactionStatus,
};

/// The subset of the callback we read. Everything else is ignored on purpose.
#[derive(Debug, Deserialize)]
pub struct Callback {
    #[serde(default)]
    pub tran_id: String,
    #[serde(default)]
    pub val_id: String,
}

fn site_url() -> String {
    std::env::var("SITE_URL").unwrap_or_else(|_| "http://localhost:8081".to_string())
}

fn redirect(path: &str) -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", format!("{}{}", site_url(), path)))
        .finish()
}

/// What happened to an order, once the gateway has been asked.
enum Outcome {
    Paid,
    /// Already settled by whichever callback arrived first.
    AlreadyPaid,
    Rejected(String),
}

/// Validate with SSLCommerz and, if the money really moved, mark the order
/// paid exactly once.
async fn settle(callback: &Callback) -> Outcome {
    if callback.val_id.trim().is_empty() || callback.tran_id.trim().is_empty() {
        return Outcome::Rejected("callback carried no transaction".to_string());
    }

    let validation = match crate::Integrations::SslCommerz::validate(&callback.val_id).await {
        Ok(validation) => validation,
        Err(message) => return Outcome::Rejected(message),
    };

    if !validation.is_successful() {
        return Outcome::Rejected(format!("gateway reported {}", validation.status));
    }

    /*
      The validation response carries its own `tran_id`. Comparing it with the
      one that arrived in the callback is what stops a valid `val_id` from one
      order being replayed against another.
    */
    if validation.tran_id != callback.tran_id {
        log::error!(
            "sslcommerz val_id {} belongs to {} but was posted for {}",
            callback.val_id, validation.tran_id, callback.tran_id
        );
        return Outcome::Rejected("transaction id mismatch".to_string());
    }

    let db = MongoDB.connect();
    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &validation.tran_id })
        .await
        .ok()
        .flatten();

    let order = match order {
        Some(order) => order,
        None => {
            log::error!("sslcommerz settled {} which is not an order", validation.tran_id);
            return Outcome::Rejected("unknown order".to_string());
        }
    };

    // Whichever callback lost the race finds the order already settled. Not an
    // error — SSLCommerz sends both, and the IPN is often first.
    if !matches!(order.payment_status, PaymentStatus::Pending) {
        return Outcome::AlreadyPaid;
    }

    // The gateway's own figure against the order's. A shortfall means the
    // amount was tampered with somewhere, and the order stays unpaid.
    let paid: Cents = match validation.amount_in_cents() {
        Some(amount) => amount,
        None => {
            log::error!("sslcommerz sent an unparseable amount: {}", validation.amount);
            return Outcome::Rejected("unreadable amount".to_string());
        }
    };

    if paid < order.totals.grand_total || validation.currency != order.currency {
        log::error!(
            "sslcommerz paid {} {} for order {} which costs {} {}",
            paid, validation.currency, order.number, order.totals.grand_total, order.currency
        );
        return Outcome::Rejected("amount or currency did not match the order".to_string());
    }

    let now = Utc::now().timestamp_millis();

    /*
      The guard on `payment_status` makes this the point of settlement: two
      callbacks racing both reach here, and only the one that matches a still
      -Pending order writes. The loser matches nothing and skips the ledger, so
      the Sale transaction cannot be written twice.
    */
    let claimed = db
        .collection::<Order>("order")
        .update_one(
            doc! { "uuid": &order.uuid, "payment_status": "Pending" },
            doc! { "$set": {
                "payment_status": mongodb::bson::to_bson(&PaymentStatus::Paid).unwrap(),
                "paid_at": now,
                "modified_at": now,
            } },
        )
        .await;

    match claimed {
        Ok(result) if result.matched_count == 0 => return Outcome::AlreadyPaid,
        Ok(_) => {}
        Err(error) => {
            log::error!("could not mark {} paid: {:?}", order.uuid, error);
            return Outcome::Rejected("could not record the payment".to_string());
        }
    }

    let transaction = PaymentTransaction {
        uuid: Uuid::now_v7().to_string(),
        order_id: order.uuid.clone(),
        user_id: order.user_id.clone(),
        kind: TransactionKind::Sale,
        status: TransactionStatus::Succeeded,
        amount: paid,
        currency: validation.currency.clone(),
        provider: "sslcommerz".to_string(),
        // The gateway's own reference, which is what support quotes back to
        // them — not a uuid we made up.
        provider_transaction_id: validation
            .bank_tran_id
            .clone()
            .unwrap_or_else(|| callback.val_id.clone()),
        method_kind: PaymentMethodKind::Card,
        payment_method_id: None,
        failure_code: None,
        failure_message: None,
        created_at: now,
        settled_at: Some(now),
    };
    if let Err(error) = db
        .collection::<PaymentTransaction>("payment_transaction")
        .insert_one(&transaction)
        .await
    {
        // The order is already Paid; losing the ledger row would make every
        // refund path under-count, so this is loud.
        log::error!("payment recorded on {} but the ledger write failed: {:?}", order.uuid, error);
    }

    if let Some(risk) = &validation.risk_level {
        if risk != "0" {
            log::warn!("sslcommerz flagged order {} at risk level {}", order.number, risk);
        }
    }

    let card = match (&validation.card_issuer, &validation.card_type) {
        (Some(issuer), Some(kind)) => format!("{} {}", issuer, kind),
        (Some(issuer), None) => issuer.clone(),
        (None, Some(kind)) => kind.clone(),
        (None, None) => "Card".to_string(),
    };
    db.collection::<Order>("order")
        .update_one(doc! { "uuid": &order.uuid }, doc! { "$set": { "payment_summary": &card } })
        .await
        .ok();

    crate::util::notify::send(
        &db,
        &order.user_id,
        crate::util::notify::Notice {
            kind: crate::model::Notification::NotifType::OrderConfirmed,
            title: format!("Payment received for order {}", order.number),
            body: "Thanks — your payment has gone through.".to_string(),
            image_id: None,
            related_id: Some(order.uuid.clone()),
            link: Some("/account/orders".to_string()),
            email: None,
        },
    ).await;

    Outcome::Paid
}

/// Release an order the shopper abandoned or that the gateway declined: put
/// the stock back, give the coupon back, and cancel it.
async fn release(order_id: &str, reason: &str) {
    let db = MongoDB.connect();

    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": order_id })
        .await
        .ok()
        .flatten();

    let order = match order {
        Some(order) => order,
        None => return,
    };

    // Only an unpaid, unshipped order. A late `fail` callback arriving after a
    // successful `success` one must not cancel a paid order.
    if !matches!(order.payment_status, PaymentStatus::Pending)
        || !matches!(order.fulfilment_status, FulfilmentStatus::Pending)
    {
        return;
    }

    let now = Utc::now().timestamp_millis();

    let claimed = db
        .collection::<Order>("order")
        .update_one(
            doc! { "uuid": order_id, "payment_status": "Pending", "fulfilment_status": "Pending" },
            doc! { "$set": {
                "fulfilment_status": mongodb::bson::to_bson(&FulfilmentStatus::Cancelled).unwrap(),
                "cancelled_at": now,
                "cancel_reason": reason,
                "modified_at": now,
            } },
        )
        .await;

    // Someone else got there first; they own the cleanup.
    if !matches!(claimed, Ok(ref r) if r.matched_count > 0) {
        return;
    }

    crate::util::order::release_order_holds(&db, order_id, now).await;
}

#[derive(Debug, Deserialize)]
pub struct CancelQuery {
    #[serde(default)]
    tran_id: String,
}

/// Browser lands here after a successful payment.
pub async fn success(form: web::Form<Callback>) -> Result<HttpResponse, Error> {
    match settle(&form).await {
        Outcome::Paid | Outcome::AlreadyPaid => {
            Ok(redirect(&format!("/checkout/confirmation?order={}", form.tran_id)))
        }
        Outcome::Rejected(reason) => {
            log::error!("sslcommerz success callback rejected for {}: {}", form.tran_id, reason);
            release(&form.tran_id, "Payment could not be verified").await;
            Ok(redirect("/checkout/payment?error=verification"))
        }
    }
}

/// The gateway declined, or the shopper's bank did.
pub async fn fail(form: web::Form<Callback>) -> Result<HttpResponse, Error> {
    release(&form.tran_id, "Payment failed").await;
    Ok(redirect("/checkout/payment?error=failed"))
}

/// The shopper backed out on the gateway's page.
pub async fn cancel(form: web::Form<Callback>) -> Result<HttpResponse, Error> {
    release(&form.tran_id, "Payment cancelled").await;
    Ok(redirect("/checkout/payment?error=cancelled"))
}

/// Server-to-server notification. Answers plain text; SSLCommerz retries on a
/// non-2xx, and a retry is harmless because `settle` is idempotent.
pub async fn ipn(form: web::Form<Callback>) -> Result<HttpResponse, Error> {
    match settle(&form).await {
        Outcome::Paid => Ok(HttpResponse::Ok().body("settled")),
        Outcome::AlreadyPaid => Ok(HttpResponse::Ok().body("already settled")),
        Outcome::Rejected(reason) => {
            log::error!("sslcommerz IPN rejected for {}: {}", form.tran_id, reason);
            // 200 regardless: the gateway retrying will not change the answer,
            // and the rejection is already in the log.
            Ok(HttpResponse::Ok().body("rejected"))
        }
    }
}

/// Not a gateway callback — the shopper's own "I gave up" from the payment
/// page, so it is authenticated and scoped to their order.
pub async fn abandon(
    req: actix_web::HttpRequest,
    query: web::Query<CancelQuery>,
) -> Result<HttpResponse, Error> {
    use crate::Middleware::Auth::{require_access, AccessRequirement};
    let user = require_access(&req, AccessRequirement::AnyToken)?;

    let db = MongoDB.connect();
    let owned = db
        .collection::<Order>("order")
        .count_documents(doc! { "uuid": &query.tran_id, "user_id": &user.user_id })
        .await
        .unwrap_or(0);

    if owned == 0 {
        return Ok(crate::util::response::Response::not_found("Order not found"));
    }

    release(&query.tran_id, "Payment abandoned").await;
    Ok(crate::util::response::Response::ok_message("Order cancelled"))
}
