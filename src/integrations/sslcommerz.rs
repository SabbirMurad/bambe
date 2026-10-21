//! SSLCommerz payment gateway.
//!
//! The flow is a hosted redirect, not a direct charge:
//!
//! 1. The order is written **unpaid** and its stock reserved, then
//!    [`init_session`] exchanges it for a `GatewayPageURL`.
//! 2. The browser is sent to that URL and pays on SSLCommerz's page. No card
//!    detail ever reaches this server, which is the point of using them.
//! 3. SSLCommerz calls back three ways: a browser form-POST to
//!    `success_url` / `fail_url` / `cancel_url`, and a server-to-server POST to
//!    `ipn_url`. All three arrive unauthenticated.
//! 4. **Nothing in a callback is trusted.** The body carries an amount and a
//!    status that anyone could forge by POSTing to the endpoint themselves, so
//!    the only thing taken from it is `val_id`, which is then presented to
//!    [`validate`] over a server-to-server call authenticated with the store
//!    password. The amount and currency that come back from *that* are the
//!    ones checked against the order.
//!
//! Configuration, all from the environment:
//!
//! - `SSLCZ_STORE_ID`, `SSLCZ_STORE_PASSWD` — from the merchant panel.
//! - `SSLCZ_SANDBOX` — "true" (default) uses the sandbox host.
//! - `SITE_URL` — the base the callbacks are built from. SSLCommerz has to be
//!   able to reach it, so `localhost` only works with the sandbox open in the
//!   same browser; a tunnel is needed for the IPN to arrive.

use std::collections::HashMap;
use std::env;
use serde::Deserialize;
use crate::model::Cents;

/// Live and sandbox differ only by host.
fn host() -> &'static str {
    let sandbox = env::var("SSLCZ_SANDBOX")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    if sandbox { "https://sandbox.sslcommerz.com" } else { "https://securepay.sslcommerz.com" }
}

fn store_id() -> Option<String> {
    env::var("SSLCZ_STORE_ID").ok().filter(|v| !v.trim().is_empty())
}

fn store_password() -> Option<String> {
    env::var("SSLCZ_STORE_PASSWD").ok().filter(|v| !v.trim().is_empty())
}

/// Whether the gateway is configured at all. Checkout offers card payment only
/// when this is true, rather than sending a shopper to a redirect that 500s.
pub fn is_configured() -> bool {
    store_id().is_some() && store_password().is_some()
}

fn site_url() -> String {
    env::var("SITE_URL").unwrap_or_else(|_| "http://localhost:8081".to_string())
}

/// What the gateway needs to know about the order it is collecting for.
pub struct SessionRequest<'a> {
    pub order_id: &'a str,
    pub order_number: &'a str,
    pub amount: Cents,
    pub currency: &'a str,
    pub customer_name: &'a str,
    pub customer_email: &'a str,
    pub customer_phone: &'a str,
    pub customer_city: &'a str,
    pub customer_country: &'a str,
    pub customer_address: &'a str,
    /// Required by SSLCommerz whenever `shipping_method` is YES, which it
    /// always is here. `zip` is optional on our own addresses, so this can be
    /// empty — the gateway takes an empty string but not a missing field.
    pub customer_postcode: &'a str,
    pub customer_state: &'a str,
    pub item_count: u32,
}

#[derive(Debug, Deserialize)]
struct SessionResponse {
    status: String,
    #[serde(rename = "GatewayPageURL")]
    gateway_page_url: Option<String>,
    failedreason: Option<String>,
}

/// Open a payment session and return the URL to send the browser to.
pub async fn init_session(request: SessionRequest<'_>) -> Result<String, String> {
    let (id, password) = match (store_id(), store_password()) {
        (Some(id), Some(password)) => (id, password),
        _ => return Err("The payment gateway isn't configured".to_string()),
    };

    let base = site_url();

    /*
      `total_amount` is major units with two decimals — the rest of this
      project keeps money in minor units, so this is one of the few places it
      is converted, and it is converted exactly once.

      `tran_id` is our order id. It comes back on every callback and is the
      only link from a payment to an order, so it must be the id and not the
      human-readable number, which a customer could guess.
    */
    let amount = format!("{}.{:02}", request.amount / 100, request.amount % 100);

    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("store_id", id);
    form.insert("store_passwd", password);
    form.insert("total_amount", amount);
    form.insert("currency", request.currency.to_string());
    form.insert("tran_id", request.order_id.to_string());

    form.insert("success_url", format!("{}/payment/sslcommerz/success", base));
    form.insert("fail_url", format!("{}/payment/sslcommerz/fail", base));
    form.insert("cancel_url", format!("{}/payment/sslcommerz/cancel", base));
    form.insert("ipn_url", format!("{}/payment/sslcommerz/ipn", base));

    form.insert("cus_name", request.customer_name.to_string());
    form.insert("cus_email", request.customer_email.to_string());
    form.insert("cus_phone", request.customer_phone.to_string());
    form.insert("cus_add1", request.customer_address.to_string());
    form.insert("cus_city", request.customer_city.to_string());
    form.insert("cus_state", request.customer_state.to_string());
    form.insert("cus_postcode", request.customer_postcode.to_string());
    form.insert("cus_country", request.customer_country.to_string());

    // Shipping block: `shipping_method` "NO" would mean a digital order, and
    // everything here is shipped.
    form.insert("shipping_method", "YES".to_string());
    form.insert("num_of_item", request.item_count.to_string());
    form.insert("ship_name", request.customer_name.to_string());
    form.insert("ship_add1", request.customer_address.to_string());
    form.insert("ship_city", request.customer_city.to_string());
    form.insert("ship_state", request.customer_state.to_string());
    form.insert("ship_postcode", request.customer_postcode.to_string());
    form.insert("ship_country", request.customer_country.to_string());

    form.insert("product_name", format!("Bambee order {}", request.order_number));
    form.insert("product_category", "Retail".to_string());
    form.insert("product_profile", "physical-goods".to_string());

    let url = format!("{}/gwprocess/v4/api.php", host());
    let response = reqwest::Client::new()
        .post(&url)
        .form(&form)
        .send()
        .await
        .map_err(|error| {
            log::error!("sslcommerz session request failed: {:?}", error);
            "Could not reach the payment gateway".to_string()
        })?;

    let body = response.text().await.map_err(|error| {
        log::error!("sslcommerz session body unreadable: {:?}", error);
        "The payment gateway sent an unreadable reply".to_string()
    })?;

    let parsed: SessionResponse = serde_json::from_str(&body).map_err(|error| {
        log::error!("sslcommerz session parse failed: {:?} — body was {}", error, body);
        "The payment gateway sent an unexpected reply".to_string()
    })?;

    if parsed.status != "SUCCESS" {
        let reason = parsed.failedreason.unwrap_or_else(|| parsed.status.clone());
        log::error!("sslcommerz refused the session: {}", reason);
        return Err(format!("The payment gateway refused the request: {}", reason));
    }

    parsed.gateway_page_url.ok_or_else(|| {
        log::error!("sslcommerz returned SUCCESS with no GatewayPageURL");
        "The payment gateway didn't return a payment page".to_string()
    })
}

/// What the validation API says about a transaction. Only these fields are
/// read; the rest of the payload is large and none of it is needed.
#[derive(Debug, Deserialize)]
pub struct Validation {
    pub status: String,
    pub tran_id: String,
    /// Major units, as a decimal string — SSLCommerz sends "1234.00".
    pub amount: String,
    pub currency: String,
    pub bank_tran_id: Option<String>,
    pub card_type: Option<String>,
    pub card_issuer: Option<String>,
    pub risk_level: Option<String>,
}

impl Validation {
    /// `VALID` for a live transaction, `VALIDATED` for a sandbox one.
    pub fn is_successful(&self) -> bool {
        self.status == "VALID" || self.status == "VALIDATED"
    }

    /// The amount back in minor units, so it can be compared with the order.
    /// Parsed rather than floated: "1234.00" times 100 as an f64 is 123399.99…
    pub fn amount_in_cents(&self) -> Option<Cents> {
        let text = self.amount.trim();
        let (whole, fraction) = match text.split_once('.') {
            Some((w, f)) => (w, f),
            None => (text, ""),
        };
        let whole: i64 = whole.parse().ok()?;
        let mut fraction = fraction.to_string();
        fraction.truncate(2);
        while fraction.len() < 2 {
            fraction.push('0');
        }
        let fraction: i64 = fraction.parse().ok()?;
        Some(whole * 100 + fraction)
    }
}

/// Ask SSLCommerz what actually happened. This is the only statement about a
/// payment this server will act on.
pub async fn validate(val_id: &str) -> Result<Validation, String> {
    let (id, password) = match (store_id(), store_password()) {
        (Some(id), Some(password)) => (id, password),
        _ => return Err("The payment gateway isn't configured".to_string()),
    };

    let url = format!("{}/validator/api/validationserverAPI.php", host());
    let response = reqwest::Client::new()
        .get(&url)
        .query(&[
            ("val_id", val_id),
            ("store_id", id.as_str()),
            ("store_passwd", password.as_str()),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|error| {
            log::error!("sslcommerz validation request failed: {:?}", error);
            "Could not reach the payment gateway".to_string()
        })?;

    let body = response.text().await.map_err(|error| {
        log::error!("sslcommerz validation body unreadable: {:?}", error);
        "The payment gateway sent an unreadable reply".to_string()
    })?;

    serde_json::from_str(&body).map_err(|error| {
        log::error!("sslcommerz validation parse failed: {:?} — body was {}", error, body);
        "The payment gateway sent an unexpected reply".to_string()
    })
}
