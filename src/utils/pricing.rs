/*
  What an order costs, in one place.

  `place_order` has to charge a number, and the checkout pages have to
  show that same number before the shopper commits to it. Two copies of
  this arithmetic — one in the handler, one in the browser — is the
  version of this bug that gets found by a customer, so the pricing is a
  pure function here and both the endpoint and the quote call it.
*/
use crate::model::Cents;
use crate::model::Order::{DeliveryMethod, PackagingOption};

#[derive(Debug, Clone)]
pub struct PriceQuote {
    pub subtotal: Cents,
    pub discount: Cents,
    pub shipping: Cents,
    pub packaging: Cents,
    pub tax: Cents,
    pub grand_total: Cents,
    // Why shipping is zero, which the page says out loud ("Free over $50")
    // rather than leaving the shopper to wonder.
    pub shipping_waived: bool,
}

pub fn totals(
    subtotal: Cents,
    discount: Cents,
    // A FreeShipping coupon zeroes the delivery line whatever it costs.
    free_shipping: bool,
    delivery: Option<&DeliveryMethod>,
    packaging: Option<&PackagingOption>,
    tax_rate_bps: i64,
    prices_include_tax: bool,
) -> PriceQuote {
    let over_threshold = delivery
        .and_then(|d| d.free_above)
        .map(|threshold| subtotal >= threshold)
        .unwrap_or(false);
    let shipping_waived = free_shipping || over_threshold;

    let shipping = match delivery {
        Some(d) if !shipping_waived => d.price,
        _ => 0,
    };
    let packaging = packaging.map(|p| p.price).unwrap_or(0);

    // Tax follows the discounted merchandise, not the shipping or the
    // gift box — the same basis the order records.
    let tax = if prices_include_tax { 0 } else { (subtotal - discount) * tax_rate_bps / 10_000 };

    PriceQuote {
        subtotal,
        discount,
        shipping,
        packaging,
        tax,
        grand_total: subtotal - discount + shipping + packaging + tax,
        shipping_waived,
    }
}
