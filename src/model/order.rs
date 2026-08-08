use serde::{Deserialize, Serialize};
use super::{Cents, CurrencyCode};
use super::address::AddressSnapshot;

/*
  Where the parcel is. This is the status the customer sees, and the one the
  admin's Fulfilment column shows.

  Kept separate from payment state because the two genuinely move
  independently: a cash-on-delivery order ships unpaid, and a refunded order
  has still been delivered.
*/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum FulfilmentStatus {
    #[default]
    Pending,
    Packing,
    Shipped,
    Delivered,
    // Stopped by staff — a stock problem, a suspected fraud check.
    OnHold,
    Cancelled,
    Returned,
}
impl std::fmt::Display for FulfilmentStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum PaymentStatus {
    #[default]
    Pending,
    // Authorised but not captured — the money is reserved, not taken.
    Authorised,
    Paid,
    Failed,
    PartiallyRefunded,
    Refunded,
}
impl std::fmt::Display for PaymentStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

//order
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Order {
    pub uuid: String,
    /*
      The human reference — "SE-2026-78432" — printed on the invoice and quoted
      to support. Separate from `uuid` because customers read this one aloud:
      it needs to be short, unique and stable, and a uuid is none of those.
    */
    pub number: String,
    pub user_id: String,

    pub items: Vec<OrderItem>,
    pub shipping_address: AddressSnapshot,
    // Usually the same as shipping; kept apart because it isn't always, and
    // the invoice needs the billing one.
    pub billing_address: AddressSnapshot,

    pub delivery: DeliveryChoice,
    pub packaging: PackagingChoice,
    pub totals: OrderTotals,
    pub currency: CurrencyCode,

    pub fulfilment_status: FulfilmentStatus,
    pub payment_status: PaymentStatus,
    // How it was paid, frozen for the invoice — "Visa ending 4242". The live
    // payment_method row can be deleted; this cannot.
    pub payment_summary: Option<String>,

    pub coupon: Option<AppliedCoupon>,
    // Gift message / delivery instructions typed at checkout.
    pub customer_note: Option<String>,
    // Internal, never shown to the customer.
    pub staff_note: Option<String>,

    pub tracking: Option<Tracking>,

    pub placed_at: i64,
    pub paid_at: Option<i64>,
    pub shipped_at: Option<i64>,
    pub delivered_at: Option<i64>,
    pub cancelled_at: Option<i64>,
    pub cancel_reason: Option<String>,
    pub modified_at: i64,
}

/*
  A purchased line.

  Every display field is copied, not referenced. The catalogue is not a stable
  record of the past: products get renamed, repriced, rephotographed and
  archived, and an order that reads through to them would quietly rewrite its
  own history — showing a total that no longer matches the sum of its lines.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderItem {
    pub product_id: String,
    pub variant_id: String,

    pub title: String,
    pub variant_label: Option<String>,   // "Black · XL"
    pub sku: String,
    pub image_id: Option<String>,
    pub brand_name: Option<String>,

    pub unit_price: Cents,
    pub quantity: u32,
    // unit_price * quantity, less any share of an item-level discount. Stored
    // rather than derived so the arithmetic on an old invoice can never change
    // under a later rounding-rule fix.
    pub line_total: Cents,

    // Set once the line is returned or refunded, so a partial return doesn't
    // need a second collection to describe it.
    pub refunded_quantity: u32,
    pub refunded_amount: Cents,
}

/*
  The money, all of it, at the moment of purchase.

  Invariant: subtotal - discount + shipping + packaging + tax == grand_total.
  Assert it before writing the order; a total that disagrees with its parts is
  the kind of bug that is only ever found by a customer.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderTotals {
    pub subtotal: Cents,
    pub discount: Cents,
    pub shipping: Cents,
    pub packaging: Cents,
    pub tax: Cents,
    pub grand_total: Cents,
}

//delivery_method — the options offered at checkout
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeliveryMethod {
    pub uuid: String,
    pub name: String,                    // "Express Delivery"
    pub description: String,             // "Estimated 1–2 business days"
    pub price: Cents,
    // Waives `price` once the subtotal clears it — the "free over $50" rule.
    pub free_above: Option<Cents>,
    pub min_days: u8,
    pub max_days: u8,
    // Same-day has a cutoff; "order by 12:00 PM", as minutes past midnight in
    // the store's timezone. None when there is no cutoff.
    pub cutoff_minutes: Option<u16>,
    pub enabled: bool,
    pub position: u16,
}

//packaging_option
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackagingOption {
    pub uuid: String,
    pub name: String,                    // "Premium Gift Packaging"
    pub description: String,
    pub price: Cents,
    pub enabled: bool,
    pub position: u16,
}

// What the customer picked, frozen into the order. Same reasoning as the
// address snapshot: retiring an option must not blank an old invoice.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeliveryChoice {
    pub method_id: String,
    pub name: String,
    pub price: Cents,
    pub estimated_from: Option<i64>,
    pub estimated_to: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackagingChoice {
    pub option_id: String,
    pub name: String,
    pub price: Cents,
    pub gift_message: Option<String>,
}

// The coupon as it was applied — code and the amount it actually took off.
// The coupon's own value can be edited later; what this order got cannot.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppliedCoupon {
    pub coupon_id: String,
    pub code: String,
    pub discount: Cents,
}

//tracking
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tracking {
    pub carrier: String,
    pub tracking_number: String,
    pub tracking_url: Option<String>,
    /*
      The shipment timeline on the tracking page. Append-only, and ordered by
      `at` rather than by position — carrier webhooks arrive out of order, and
      sorting by arrival puts "Out for delivery" above "Delivered".
    */
    pub events: Vec<ShipmentEvent>,
    pub estimated_delivery: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShipmentEvent {
    pub status: String,                  // "Out for delivery"
    pub description: Option<String>,
    pub location: Option<String>,
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ReturnStatus {
    #[default]
    Requested,
    Approved,
    Rejected,
    // Customer has sent it; not yet with us.
    InTransit,
    Received,
    Refunded,
}
impl std::fmt::Display for ReturnStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  return_request — its own document, not a flag on the order.

  A return has a lifecycle of its own (requested, approved, received,
  refunded), it can cover part of an order, and there can be more than one
  against the same order. None of that fits in an order field.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReturnRequest {
    pub uuid: String,
    pub order_id: String,
    pub user_id: String,
    pub items: Vec<ReturnItem>,
    pub reason: String,
    pub details: Option<String>,
    // Photos of a damaged item — ImageStruct uuids.
    pub image_ids: Vec<String>,
    pub status: ReturnStatus,
    pub refund_amount: Option<Cents>,
    pub staff_note: Option<String>,
    pub requested_at: i64,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReturnItem {
    pub variant_id: String,
    pub quantity: u32,
}
