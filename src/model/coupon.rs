use serde::{Deserialize, Serialize};
use super::Cents;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiscountType {
    // `value` is basis points of the subtotal: 2000 = 20%.
    Percentage,
    // `value` is a flat amount in cents.
    Fixed,
    // `value` is ignored; the shipping line is zeroed.
    FreeShipping,
}
impl std::fmt::Display for DiscountType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  coupon — a discount code.

  There is no stored `status`. The admin list shows Active, Scheduled, Expired,
  Used up and Disabled, and every one of those is derived from the switch, the
  dates and the redemption count. Writing it down would mean a cron job to flip
  a code to "expired" at midnight, and a code that stays redeemable for however
  long that job is late.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Coupon {
    pub uuid: String,
    // Uppercase, unique. Match case-insensitively on redemption — customers
    // type it as they please.
    pub code: String,
    pub description: String,

    pub discount_type: DiscountType,
    // Read against `discount_type`: basis points, or cents.
    pub value: i64,
    // Cap on a percentage discount — "20% off, up to $50".
    pub max_discount: Option<Cents>,
    pub min_spend: Option<Cents>,

    // None means no ceiling.
    pub total_limit: Option<u32>,
    pub per_user_limit: Option<u32>,
    /*
      Denormalised count of coupon_redemption rows. Increment atomically inside
      the same transaction as the redemption — a read-then-write here is the
      race that lets a "100 uses" code go out 300 times in a flash sale.
    */
    pub used_count: u32,

    pub starts_at: i64,
    // None runs forever.
    pub ends_at: Option<i64>,
    // The admin's on/off switch, independent of the dates.
    pub enabled: bool,

    // Empty means the whole catalogue.
    pub product_ids: Vec<String>,
    pub category_ids: Vec<String>,
    // Restricts the code to accounts with no completed order — the welcome
    // offer.
    pub first_order_only: bool,
    // A personal code — a loyalty reward — that only this account can use.
    #[serde(default)]
    pub user_id: Option<String>,

    pub created_at: i64,
    pub created_by: String,
    pub modified_at: i64,
}

/*
  coupon_redemption — one row per successful use.

  This is what `per_user_limit` is actually checked against, and what survives
  when someone edits the coupon afterwards. Unique index on
  (coupon_id, order_id) so a retried checkout cannot record the same use twice.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CouponRedemption {
    pub uuid: String,
    pub coupon_id: String,
    pub code: String,
    pub user_id: String,
    pub order_id: String,
    // What it actually took off this order.
    pub discount: Cents,
    pub redeemed_at: i64,
}
