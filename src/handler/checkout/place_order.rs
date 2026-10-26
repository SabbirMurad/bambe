use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::require_shopper;
use crate::model::Cart::{Cart, CartItem};
use crate::model::Product::{Product, ProductVariant, Brand, StockMovement, StockReason};
use crate::model::Address::{Address, AddressSnapshot};
use crate::model::Order::{
    Order, OrderItem, OrderTotals, DeliveryChoice, DeliveryMethod,
    PackagingChoice, PackagingOption, AppliedCoupon, FulfilmentStatus, PaymentStatus,
};
use crate::model::Store::StoreSettings;
use crate::model::Coupon::CouponRedemption;
use crate::model::Payment::{PaymentMethodKind, PaymentMethod};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    address_id: String,
    billing_address_id: Option<String>,
    delivery_method_id: String,
    packaging_option_id: Option<String>,
    gift_message: Option<String>,
    payment_method_kind: PaymentMethodKind,
    // A saved PaymentMethod to charge. None for cash on delivery, or a
    // one-off card the client tokenised without saving it.
    payment_method_id: Option<String>,
    customer_note: Option<String>,
}

fn snapshot(a: &Address) -> AddressSnapshot {
    AddressSnapshot {
        address_id: Some(a.uuid.clone()),
        label: a.label.clone(),
        recipient_name: a.recipient_name.clone(),
        street: a.street.clone(),
        city: a.city.clone(),
        state: a.state.clone(),
        zip: a.zip.clone(),
        country: a.country.clone(),
        phone: a.phone.clone(),
        note: a.note.clone(),
    }
}

/// Put stock back for a checkout that took it and then failed. Best-effort and
/// deliberately unconditional: the alternative to a release that errors is an
/// order which never existed holding units for good.
async fn release_stock(db: &mongodb::Database, items: &[CartItem], now: i64) {
    for item in items {
        db.collection::<ProductVariant>("product_variant")
            .update_one(
                doc! { "uuid": &item.variant_id },
                doc! { "$inc": { "stock": item.quantity as i32 }, "$set": { "modified_at": now } },
            )
            .await
            .ok();
    }
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_shopper(&req)?;
    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();

    let cart = match db.collection::<Cart>("cart").find_one(doc! { "user_id": &user.user_id }).await.ok().flatten() {
        Some(c) => c,
        None => return Ok(Response::bad_request("Your cart is empty")),
    };

    let mut item_cursor = db
        .collection::<CartItem>("cart_item")
        .find(doc! { "cart_id": &cart.uuid })
        .await
        .expect("cart item lookup");
    let mut cart_items = Vec::new();
    while let Some(result) = item_cursor.next().await {
        let i = match result {
            Ok(i) => i,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        cart_items.push(i);
    }
    if cart_items.is_empty() {
        return Ok(Response::bad_request("Your cart is empty"));
    }

    let variant_ids: Vec<String> = cart_items.iter().map(|i| i.variant_id.clone()).collect();
    let mut variant_cursor = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "uuid": { "$in": &variant_ids } })
        .await
        .expect("variant lookup");
    let mut variants: HashMap<String, ProductVariant> = HashMap::new();
    while let Some(result) = variant_cursor.next().await {
        let v = match result {
            Ok(v) => v,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        variants.insert(v.uuid.clone(), v);
    }

    let product_ids: Vec<String> = cart_items.iter().map(|i| i.product_id.clone()).collect();
    let mut product_cursor = db
        .collection::<Product>("product")
        .find(doc! { "uuid": { "$in": &product_ids } })
        .await
        .expect("product lookup");
    let mut products: HashMap<String, Product> = HashMap::new();
    while let Some(result) = product_cursor.next().await {
        let p = match result {
            Ok(p) => p,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        products.insert(p.uuid.clone(), p);
    }

    // Every line has to still resolve before anything is written — a checkout
    // can't half-succeed. The stock test here is only a fast, friendly reject:
    // the binding check is the conditional `$inc` further down, because
    // anything read here can be sold by another request before we write.
    for item in &cart_items {
        let variant = match variants.get(&item.variant_id) {
            Some(v) => v,
            None => return Ok(Response::bad_request("An item in your cart is no longer available")),
        };
        if !products.contains_key(&item.product_id) {
            return Ok(Response::bad_request("An item in your cart is no longer available"));
        }
        if variant.stock < item.quantity as i32 {
            let title = products.get(&item.product_id).map(|p| p.title.as_str()).unwrap_or("An item");
            return Ok(Response::bad_request(&format!("{} doesn't have enough stock left", title)));
        }
    }

    let brand_ids: Vec<String> = products.values().filter_map(|p| p.brand_id.clone()).collect();
    let brands: HashMap<String, Brand> = if brand_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db.collection::<Brand>("brand").find(doc! { "uuid": { "$in": &brand_ids } }).await.expect("brand lookup");
        let mut map = HashMap::new();
        while let Some(result) = cursor.next().await {
            let b = match result {
                Ok(b) => b,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            map.insert(b.uuid.clone(), b);
        }
        map
    };

    let subtotal: i64 = cart_items.iter().map(|i| variants[&i.variant_id].price * i.quantity as i64).sum();

    let shipping_address = db
        .collection::<Address>("address")
        .find_one(doc! { "uuid": &body.address_id, "user_id": &user.user_id, "deleted_at": null })
        .await
        .ok()
        .flatten();
    let shipping_address = match shipping_address {
        Some(a) => a,
        None => return Ok(Response::bad_request("Delivery address not found")),
    };

    // Omitting a billing address means "same as shipping"; naming one that
    // can't be resolved is a mistake worth reporting. Falling back silently
    // billed the shipping address under a different address's id, and the
    // order's own snapshot was the only place that showed it.
    let billing_address = match &body.billing_address_id {
        Some(id) => {
            let found = db
                .collection::<Address>("address")
                .find_one(doc! { "uuid": id, "user_id": &user.user_id, "deleted_at": null })
                .await
                .ok()
                .flatten();
            match found {
                Some(address) => address,
                None => return Ok(Response::bad_request("Billing address not found")),
            }
        }
        None => shipping_address.clone(),
    };

    let delivery_method = db
        .collection::<DeliveryMethod>("delivery_method")
        .find_one(doc! { "uuid": &body.delivery_method_id, "enabled": true })
        .await
        .ok()
        .flatten();
    let delivery_method = match delivery_method {
        Some(m) => m,
        None => return Ok(Response::bad_request("Delivery method not found")),
    };

    let packaging_option = match &body.packaging_option_id {
        Some(id) => db.collection::<PackagingOption>("packaging_option").find_one(doc! { "uuid": id, "enabled": true }).await.ok().flatten(),
        None => None,
    };
    let unit_count: u32 = cart_items.iter().map(|i| i.quantity).sum();
    if packaging_option.as_ref().map(|p| p.multi_item_only && unit_count < 2).unwrap_or(false) {
        return Ok(Response::bad_request("One-box packaging needs two or more items in the cart"));
    }

    // Coupon is re-evaluated here rather than trusted from the cart — it
    // can expire or hit its limit in the minutes between browsing the cart
    // and placing the order. A code that's gone stale is just dropped
    // rather than blocking checkout over a promo.
    let applied_discount = match &cart.coupon_code {
        Some(code) => crate::util::coupon::evaluate(&db, code, Some(&user.user_id), subtotal, &product_ids).await.ok(),
        None => None,
    };
    let discount = applied_discount.as_ref().map(|a| a.discount).unwrap_or(0);
    let free_shipping = applied_discount.as_ref().map(|a| a.free_shipping).unwrap_or(false);

    let store_settings = crate::Handler::Admin::Settings::load(&db).await;
    let currency = store_settings.as_ref().map(|s| s.default_currency.clone()).unwrap_or_else(|| "BDT".to_string());
    let order_number_prefix = store_settings.as_ref().map(|s| s.order_number_prefix.clone()).unwrap_or_else(|| "OR".to_string());
    let tax_rate_bps = store_settings.as_ref().map(|s| s.tax_rate_bps as i64).unwrap_or(0);
    let prices_include_tax = store_settings.as_ref().map(|s| s.prices_include_tax).unwrap_or(false);

    // Shared with GET /v1/checkout/quote, which is what the review page
    // showed the shopper a moment ago. One function, one answer.
    let quote = crate::util::pricing::totals(
        subtotal,
        discount,
        free_shipping,
        Some(&delivery_method),
        packaging_option.as_ref(),
        tax_rate_bps,
        prices_include_tax,
    );
    let shipping_price = quote.shipping;
    let packaging_price = quote.packaging;
    let tax = quote.tax;
    let grand_total = quote.grand_total;

    let totals = OrderTotals {
        subtotal,
        discount,
        shipping: shipping_price,
        packaging: packaging_price,
        tax,
        grand_total,
    };

    let delivery = DeliveryChoice {
        method_id: delivery_method.uuid.clone(),
        name: delivery_method.name.clone(),
        price: shipping_price,
        estimated_from: Some(now + delivery_method.min_days as i64 * DAY_MS),
        estimated_to: Some(now + delivery_method.max_days as i64 * DAY_MS),
    };

    let packaging = match &packaging_option {
        Some(p) => PackagingChoice {
            option_id: p.uuid.clone(),
            name: p.name.clone(),
            price: p.price,
            gift_message: body.gift_message.clone(),
        },
        // No gift packaging picked — still a valid, zero-cost choice rather
        // than an absent field, matching every other "choice" on the order.
        None => PackagingChoice {
            option_id: String::new(),
            name: "No packaging".to_string(),
            price: 0,
            gift_message: body.gift_message.clone(),
        },
    };

    /*
      An order is never written as paid here.

      Cash on delivery stays Pending until the courier collects. Everything
      else goes through SSLCommerz: the order is written unpaid, its stock is
      reserved, and the shopper is redirected to the gateway. Only the
      validated callback in `handler/payment/sslcommerz` flips it to Paid and
      writes the Sale transaction — so no order can claim to be paid on this
      server's say-so.
    */
    let is_cod = body.payment_method_kind == PaymentMethodKind::CashOnDelivery;
    if !is_cod && !crate::Integrations::SslCommerz::is_configured() {
        return Ok(Response::bad_request(
            "Card payment isn't available right now — please choose cash on delivery",
        ));
    }
    let payment_status = PaymentStatus::Pending;
    let paid_at: Option<i64> = None;

    let saved_method = match &body.payment_method_id {
        Some(id) => db.collection::<PaymentMethod>("payment_method").find_one(doc! { "uuid": id, "user_id": &user.user_id }).await.ok().flatten(),
        None => None,
    };
    let payment_summary = if is_cod {
        Some("Cash on Delivery".to_string())
    } else if let Some(m) = &saved_method {
        match (&m.card_brand, &m.last4) {
            (Some(brand), Some(last4)) => Some(format!("{} ending {}", brand, last4)),
            _ => m.account_email.clone().or_else(|| Some(body.payment_method_kind.to_string())),
        }
    } else {
        Some(body.payment_method_kind.to_string())
    };

    // Take the stock before the order exists. `$inc` guarded by `stock: {$gte}`
    // makes the test and the decrement one atomic operation, so two checkouts
    // racing for the last unit cannot both win — the loser matches no document
    // and gets `None` back. `balances[i]` is the post-decrement stock straight
    // from the write, which is what the movement log has to record.
    let mut balances: Vec<i32> = Vec::new();
    let mut shortfall: Option<String> = None;

    for item in &cart_items {
        let taken = db
            .collection::<ProductVariant>("product_variant")
            .find_one_and_update(
                doc! { "uuid": &item.variant_id, "stock": { "$gte": item.quantity as i32 } },
                doc! { "$inc": { "stock": -(item.quantity as i32) }, "$set": { "modified_at": now } },
            )
            .return_document(ReturnDocument::After)
            .await;

        match taken {
            Ok(Some(variant)) => balances.push(variant.stock),
            Ok(None) => {
                let title = products.get(&item.product_id).map(|p| p.title.as_str()).unwrap_or("An item");
                shortfall = Some(format!("{} doesn't have enough stock left", title));
                break;
            }
            Err(error) => {
                log::error!("{:?}", error);
                shortfall = Some("Could not reserve stock for your order".to_string());
                break;
            }
        }
    }

    if let Some(message) = shortfall {
        release_stock(&db, &cart_items[..balances.len()], now).await;
        return Ok(Response::bad_request(&message));
    }

    let order_id = Uuid::now_v7().to_string();

    // A per-year counter, not a random 5-digit number: 90k values with no
    // uniqueness check collide about half the time by ~350 orders a year, and
    // `order.number` is what the customer quotes to support. The counter is one
    // atomic `$inc`, so concurrent checkouts can't draw the same seq.
    let year = chrono::Utc::now().format("%Y").to_string();
    let seq = match crate::util::counter::next(&db, &format!("order-number-{}", year)).await {
        Some(seq) => seq,
        None => {
            release_stock(&db, &cart_items, now).await;
            return Ok(Response::internal_server_error("Could not allocate an order number"));
        }
    };
    let order_number = format!("{}-{}-{:05}", order_number_prefix, year, seq);

    let order = Order {
        uuid: order_id.clone(),
        number: order_number,
        user_id: user.user_id.clone(),
        shipping_address: snapshot(&shipping_address),
        billing_address: snapshot(&billing_address),
        delivery,
        packaging,
        totals,
        currency: currency.clone(),
        fulfilment_status: FulfilmentStatus::Pending,
        payment_status: payment_status.clone(),
        payment_summary,
        coupon: applied_discount.as_ref().map(|a| AppliedCoupon {
            coupon_id: a.coupon.uuid.clone(),
            code: a.coupon.code.clone(),
            discount: a.discount,
        }),
        customer_note: body.customer_note.clone(),
        staff_note: None,
        tracking: None,
        placed_at: now,
        paid_at,
        shipped_at: None,
        delivered_at: None,
        cancelled_at: None,
        cancel_reason: None,
        modified_at: now,
    };

    if let Err(error) = db.collection::<Order>("order").insert_one(&order).await {
        log::error!("{:?}", error);
        release_stock(&db, &cart_items, now).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let order_items: Vec<OrderItem> = cart_items
        .iter()
        .map(|item| {
            let variant = &variants[&item.variant_id];
            let product = &products[&item.product_id];
            // "Shade: Ruby Woo · Volume: 3 g", frozen with the order.
            let variant_label = variant.label();
            let unit_price = variant.price;
            let line_total = unit_price * item.quantity as i64;

            OrderItem {
                product_id: item.product_id.clone(),
                variant_id: item.variant_id.clone(),
                title: product.title.clone(),
                variant_label,
                sku: variant.sku.clone(),
                // Snapshotted from the variant bought, so the order still shows
                // the right colourway after the catalogue moves on.
                image_id: Some(variant.image_id.clone()),
                brand_name: product.brand_id.as_ref().and_then(|id| brands.get(id)).map(|b| b.name.clone()),
                unit_price,
                quantity: item.quantity,
                line_total,
                refunded_quantity: 0,
                refunded_amount: 0,
                uuid: Uuid::now_v7().to_string(),
                order_id: order_id.clone(),
            }
        })
        .collect();

    if let Err(error) = db.collection::<OrderItem>("order_item").insert_many(&order_items).await {
        log::error!("{:?}", error);
        db.collection::<Order>("order").delete_one(doc! { "uuid": &order_id }).await.ok();
        release_stock(&db, &cart_items, now).await;
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // Stock was already taken above; this only records the movement and rolls
    // the product's aggregates (sold_count, in_stock) forward from it.
    for (index, item) in cart_items.iter().enumerate() {
        let new_stock = balances[index];

        let movement = StockMovement {
            uuid: Uuid::now_v7().to_string(),
            variant_id: item.variant_id.clone(),
            delta: -(item.quantity as i32),
            balance_after: new_stock,
            reason: StockReason::Sale,
            order_id: Some(order_id.clone()),
            note: None,
            created_by: None,
            created_at: now,
        };
        db.collection::<StockMovement>("stock_movement").insert_one(&movement).await.ok();

        db.collection::<Product>("product")
            .update_one(doc! { "uuid": &item.product_id }, doc! { "$inc": { "sold_count": item.quantity as i64 } })
            .await
            .ok();
    }

    for product_id in products.keys() {
        let any_in_stock = db
            .collection::<ProductVariant>("product_variant")
            .count_documents(doc! { "product_id": product_id, "stock": { "$gt": 0 } })
            .await
            .unwrap_or(1)
            > 0;
        db.collection::<Product>("product")
            .update_one(doc! { "uuid": product_id }, doc! { "$set": { "in_stock": any_in_stock } })
            .await
            .ok();
    }

    if let Some(applied) = &applied_discount {
        db.collection::<crate::model::Coupon::Coupon>("coupon")
            .update_one(doc! { "uuid": &applied.coupon.uuid }, doc! { "$inc": { "used_count": 1 } })
            .await
            .ok();

        let redemption = CouponRedemption {
            uuid: Uuid::now_v7().to_string(),
            coupon_id: applied.coupon.uuid.clone(),
            code: applied.coupon.code.clone(),
            user_id: user.user_id.clone(),
            order_id: order_id.clone(),
            discount: applied.discount,
            redeemed_at: now,
        };
        db.collection::<CouponRedemption>("coupon_redemption").insert_one(&redemption).await.ok();
    }

    /*
      No Sale transaction is written here. For cash on delivery there is
      nothing to record until the money is collected; for a card the gateway's
      validated callback writes it. `payment_transaction` is the ledger every
      refund path sums, so a row in it has to mean money actually moved.
    */

    crate::util::customer::roll_up_purchase(&db, &user.user_id, grand_total, now).await;

    // The cart is converted, not deleted — it's the denominator behind
    // abandonment stats, and this order is what it converted into.
    db.collection::<CartItem>("cart_item").delete_many(doc! { "cart_id": &cart.uuid }).await.ok();
    db.collection::<Cart>("cart")
        .update_one(
            doc! { "uuid": &cart.uuid },
            doc! { "$set": { "coupon_code": mongodb::bson::Bson::Null, "converted_order_id": &order_id, "modified_at": now } },
        )
        .await
        .ok();

    // The bell and the confirmation email. Best-effort by design — the order is
    // already committed, and a mail outage must not turn a placed order into a
    // 500 the shopper retries.
    let item_count: u32 = cart_items.iter().map(|i| i.quantity).sum();
    // SSLCommerz requires a customer email on the session.
    let customer_email = db
        .collection::<crate::Model::Account::AccountCore>("account_core")
        .find_one(doc! { "uuid": &user.user_id })
        .await
        .ok()
        .flatten()
        .map(|a| a.email_address)
        .unwrap_or_default();
    let summary = order.payment_summary.clone().unwrap_or_else(|| "—".to_string());
    crate::util::notify::send(
        &db,
        &user.user_id,
        crate::util::notify::Notice {
            kind: crate::model::Notification::NotifType::OrderPlaced,
            title: format!("Order {} placed", order.number),
            body: format!("We've received your order of {} item(s).", item_count),
            image_id: order_items.first().and_then(|i| i.image_id.clone()),
            related_id: Some(order_id.clone()),
            link: Some("/account/orders".to_string()),
            email: Some(crate::util::notify::Email {
                subject: format!("Your Bambee order {}", order.number),
                html: crate::Integrations::Resend::order_placed_template(
                    &order.number,
                    item_count,
                    grand_total,
                    &order.currency,
                    &summary,
                    &order.delivery.name,
                ),
            }),
        },
    ).await;

    /*
      Open the gateway session last, once the order is committed and the cart
      converted — the session's `tran_id` is this order's id, so the order has
      to exist first.

      A failure here is not fatal to the order: it stays Pending and unpaid,
      exactly as an abandoned payment would, and the shopper is told. Cleaning
      it up is the abandonment path's job, not this one's.
    */
    let payment_url = if is_cod {
        None
    } else {
        let request = crate::Integrations::SslCommerz::SessionRequest {
            order_id: &order_id,
            order_number: &order.number,
            amount: grand_total,
            currency: &order.currency,
            customer_name: &order.shipping_address.recipient_name,
            customer_email: &customer_email,
            customer_phone: &order.shipping_address.phone,
            customer_city: &order.shipping_address.city,
            customer_country: &order.shipping_address.country,
            customer_address: &order.shipping_address.street,
            customer_postcode: order.shipping_address.zip.as_deref().unwrap_or(""),
            customer_state: order.shipping_address.state.as_deref().unwrap_or(""),
            item_count,
        };

        match crate::Integrations::SslCommerz::init_session(request).await {
            Ok(url) => Some(url),
            Err(message) => {
                log::error!("could not open a payment session for {}: {}", order_id, message);
                None
            }
        }
    };

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "order": order,
        // Where the browser has to go next. Null for cash on delivery, and
        // null if the gateway could not be reached — the checkout page shows
        // the order as placed-but-unpaid rather than pretending otherwise.
        "payment_url": payment_url,
        "requires_payment": !is_cod,
    })))
}
