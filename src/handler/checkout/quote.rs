/*
  What this cart would cost, given the choices made so far.

  The three checkout pages all show a running total, and until now
  nothing could tell them one: `GET /v1/cart` knows the merchandise and
  the coupon, but shipping comes from the delivery method and tax from
  the store's rate, neither of which the browser has any business
  knowing. So the browser stops guessing and asks.

  Read-only, and safe to call on every click of a delivery option. It
  shares `utils::pricing::totals` with `place_order`, which is the whole
  point — the number shown here is the number that will be charged.
*/
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::model::Order::{DeliveryMethod, PackagingOption};
use crate::model::Store::StoreSettings;
use crate::utils::pricing;
use crate::Handler::Cart::{resolve_owner, find_cart, build_response, compute_subtotal, Owner};

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    delivery_method_id: Option<String>,
    packaging_option_id: Option<String>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let cart = match resolve_owner(&req) {
        Some(owner) => find_cart(&db, &owner).await.map(|c| (owner, c)),
        None => None,
    };

    let store = crate::Handler::Admin::Settings::load(&db).await;
    let tax_rate_bps = store.as_ref().map(|s| s.tax_rate_bps as i64).unwrap_or(0);
    let prices_include_tax = store.as_ref().map(|s| s.prices_include_tax).unwrap_or(false);
    let currency = store.as_ref().map(|s| s.default_currency.clone()).unwrap_or_else(|| "USD".to_string());

    let delivery = match &query.delivery_method_id {
        Some(id) if !id.is_empty() => db
            .collection::<DeliveryMethod>("delivery_method")
            .find_one(doc! { "uuid": id, "enabled": true })
            .await
            .ok()
            .flatten(),
        _ => None,
    };

    let packaging = match &query.packaging_option_id {
        Some(id) if !id.is_empty() => db
            .collection::<PackagingOption>("packaging_option")
            .find_one(doc! { "uuid": id, "enabled": true })
            .await
            .ok()
            .flatten(),
        _ => None,
    };

    let (owner, cart) = match cart {
        Some(pair) => pair,
        // No cart yet is a perfectly ordinary state — quote it as zero
        // rather than as an error the page has to special-case.
        None => {
            let quote = pricing::totals(0, 0, false, delivery.as_ref(), packaging.as_ref(), tax_rate_bps, prices_include_tax);
            return Ok(HttpResponse::Ok().content_type("application/json").json(json!({
                "items": [],
                "item_count": 0,
                "subtotal": 0,
                "discount": 0,
                "coupon_code": null,
                "coupon_error": null,
                "shipping": quote.shipping,
                "shipping_waived": quote.shipping_waived,
                "packaging": quote.packaging,
                "tax": quote.tax,
                "tax_rate_bps": tax_rate_bps,
                "grand_total": quote.grand_total,
                "currency": currency,
                "delivery_method": null,
                "packaging_option": null,
            })));
        }
    };

    let body = build_response(&db, &cart).await;
    let subtotal = body["subtotal"].as_i64().unwrap_or(0);
    let discount = body["discount"].as_i64().unwrap_or(0);

    // The coupon is evaluated again rather than read off the cart body,
    // because free shipping is a property of the code, not a number, and
    // `build_response` only reports the money it took off.
    let free_shipping = match &cart.coupon_code {
        Some(code) => {
            let (sub, product_ids) = compute_subtotal(&db, &cart.uuid).await;
            let user_id = match &owner {
                Owner::User(id) => Some(id.as_str()),
                Owner::Guest(_) => None,
            };
            crate::utils::coupon::evaluate(&db, code, user_id, sub, &product_ids)
                .await
                .map(|applied| applied.free_shipping)
                .unwrap_or(false)
        }
        None => false,
    };

    let quote = pricing::totals(
        subtotal,
        discount,
        free_shipping,
        delivery.as_ref(),
        packaging.as_ref(),
        tax_rate_bps,
        prices_include_tax,
    );

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": body["items"],
        "item_count": body["item_count"],
        "subtotal": quote.subtotal,
        "discount": quote.discount,
        "coupon_code": body["coupon_code"],
        "coupon_error": body["coupon_error"],
        "shipping": quote.shipping,
        "shipping_waived": quote.shipping_waived,
        "packaging": quote.packaging,
        "tax": quote.tax,
        "tax_rate_bps": tax_rate_bps,
        "grand_total": quote.grand_total,
        "currency": currency,
        "delivery_method": delivery.as_ref().map(|d| json!({
            "uuid": d.uuid, "name": d.name, "price": d.price,
            "min_days": d.min_days, "max_days": d.max_days,
        })),
        "packaging_option": packaging.as_ref().map(|p| json!({
            "uuid": p.uuid, "name": p.name, "price": p.price,
        })),
        /*
          Whether the payment step can offer anything but cash on delivery.
          Without gateway credentials `place_order` refuses a card, so the
          option is hidden rather than shown and then rejected.
        */
        "card_payment_available": crate::Integrations::SslCommerz::is_configured(),
    })))
}
