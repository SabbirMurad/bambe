use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;
use rand::Rng;
use futures::StreamExt;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Cart::{Cart, CartItem};
use crate::model::Product::{Product, ProductVariant, Brand, StockMovement, StockReason};
use crate::model::Address::{Address, AddressSnapshot};
use crate::model::Order::{
    Order, OrderItem, OrderTotals, DeliveryChoice, DeliveryMethod,
    PackagingChoice, PackagingOption, AppliedCoupon, FulfilmentStatus, PaymentStatus,
};
use crate::model::Store::StoreSettings;
use crate::model::Coupon::CouponRedemption;
use crate::model::Payment::{PaymentTransaction, PaymentMethodKind, PaymentMethod, TransactionKind, TransactionStatus};

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

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
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
    while let Some(Ok(i)) = item_cursor.next().await {
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
    while let Some(Ok(v)) = variant_cursor.next().await {
        variants.insert(v.uuid.clone(), v);
    }

    let product_ids: Vec<String> = cart_items.iter().map(|i| i.product_id.clone()).collect();
    let mut product_cursor = db
        .collection::<Product>("product")
        .find(doc! { "uuid": { "$in": &product_ids } })
        .await
        .expect("product lookup");
    let mut products: HashMap<String, Product> = HashMap::new();
    while let Some(Ok(p)) = product_cursor.next().await {
        products.insert(p.uuid.clone(), p);
    }

    // Every line has to still resolve and still have the stock it asks for
    // before anything is written — a checkout can't half-succeed.
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
        while let Some(Ok(b)) = cursor.next().await {
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

    let billing_address = match &body.billing_address_id {
        Some(id) => db
            .collection::<Address>("address")
            .find_one(doc! { "uuid": id, "user_id": &user.user_id, "deleted_at": null })
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| shipping_address.clone()),
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

    // Coupon is re-evaluated here rather than trusted from the cart — it
    // can expire or hit its limit in the minutes between browsing the cart
    // and placing the order. A code that's gone stale is just dropped
    // rather than blocking checkout over a promo.
    let applied_discount = match &cart.coupon_code {
        Some(code) => crate::utils::coupon::evaluate(&db, code, Some(&user.user_id), subtotal, &product_ids).await.ok(),
        None => None,
    };
    let discount = applied_discount.as_ref().map(|a| a.discount).unwrap_or(0);
    let free_shipping = applied_discount.as_ref().map(|a| a.free_shipping).unwrap_or(false);

    let store_settings = db.collection::<StoreSettings>("store_settings").find_one(doc! {}).await.ok().flatten();
    let currency = store_settings.as_ref().map(|s| s.default_currency.clone()).unwrap_or_else(|| "USD".to_string());
    let order_number_prefix = store_settings.as_ref().map(|s| s.order_number_prefix.clone()).unwrap_or_else(|| "OR".to_string());
    let tax_rate_bps = store_settings.as_ref().map(|s| s.tax_rate_bps as i64).unwrap_or(0);
    let prices_include_tax = store_settings.as_ref().map(|s| s.prices_include_tax).unwrap_or(false);

    let shipping_waived = free_shipping || delivery_method.free_above.map(|t| subtotal >= t).unwrap_or(false);
    let shipping_price = if shipping_waived { 0 } else { delivery_method.price };
    let packaging_price = packaging_option.as_ref().map(|p| p.price).unwrap_or(0);
    let tax = if prices_include_tax { 0 } else { (subtotal - discount) * tax_rate_bps / 10_000 };
    let grand_total = subtotal - discount + shipping_price + packaging_price + tax;

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

    // No payment gateway is wired in yet: cash on delivery is left unpaid
    // until collected, and every other method is recorded as an immediate
    // mock success so the rest of the order lifecycle can be built and
    // tested against. Swap this block out once a real processor is chosen.
    let is_cod = body.payment_method_kind == PaymentMethodKind::CashOnDelivery;
    let payment_status = if is_cod { PaymentStatus::Pending } else { PaymentStatus::Paid };
    let paid_at = if is_cod { None } else { Some(now) };

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

    let order_id = Uuid::now_v7().to_string();
    let order_number = format!(
        "{}-{}-{:05}",
        order_number_prefix,
        chrono::Utc::now().format("%Y"),
        rand::rng().random_range(10_000..99_999),
    );

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
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let order_items: Vec<OrderItem> = cart_items
        .iter()
        .map(|item| {
            let variant = &variants[&item.variant_id];
            let product = &products[&item.product_id];
            let variant_label = match (&variant.color, &variant.size) {
                (Some(c), Some(s)) => Some(format!("{} · {}", c, s)),
                (Some(c), None) => Some(c.clone()),
                (None, Some(s)) => Some(s.clone()),
                (None, None) => None,
            };
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
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // Decrement stock and log the movement, then roll the product's
    // aggregates (sold_count, in_stock) forward from it.
    for item in &cart_items {
        let variant = &variants[&item.variant_id];
        let new_stock = variant.stock - item.quantity as i32;

        db.collection::<ProductVariant>("product_variant")
            .update_one(doc! { "uuid": &item.variant_id }, doc! { "$set": { "stock": new_stock, "modified_at": now } })
            .await
            .ok();

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

    if !is_cod {
        let transaction = PaymentTransaction {
            uuid: Uuid::now_v7().to_string(),
            order_id: order_id.clone(),
            user_id: user.user_id.clone(),
            kind: TransactionKind::Sale,
            status: TransactionStatus::Succeeded,
            amount: grand_total,
            currency,
            provider: "mock".to_string(),
            provider_transaction_id: Uuid::now_v7().to_string(),
            method_kind: body.payment_method_kind.clone(),
            payment_method_id: body.payment_method_id.clone(),
            failure_code: None,
            failure_message: None,
            created_at: now,
            settled_at: Some(now),
        };
        db.collection::<PaymentTransaction>("payment_transaction").insert_one(&transaction).await.ok();
    }

    crate::utils::customer::roll_up_purchase(&db, &user.user_id, grand_total, now).await;

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

    Ok(HttpResponse::Ok().content_type("application/json").json(order))
}
