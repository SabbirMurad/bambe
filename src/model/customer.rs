use serde::{Deserialize, Serialize};
use super::Cents;

/*
  Which bucket the admin's customer list puts someone in. Derived on a
  schedule from order history, not set by hand:
    New      — one order, or none yet
    Repeat   — more than one
    Vip      — past the shop's lifetime-spend threshold
    Dormant  — has ordered, but not for a long while
*/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum CustomerSegment {
    #[default]
    New,
    Repeat,
    Vip,
    Dormant,
}
impl std::fmt::Display for CustomerSegment {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  customer_profile — the commerce half of an account.

  Deliberately not merged into AccountCore: everything here is a rollup of
  orders that gets rewritten on every purchase, while the identity fields are
  written once. Splitting them keeps the sign-in path off the write path of the
  checkout.

  These are reporting figures. Never charge, refund or gate anything on
  `lifetime_spend` — read the orders for that.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomerProfile {
    pub uuid: String,
    pub user_id: String,

    pub segment: CustomerSegment,
    pub order_count: u32,
    pub lifetime_spend: Cents,
    pub last_order_at: Option<i64>,
    // From the default address — the admin list's Location column.
    pub location: Option<String>,

    pub loyalty_points: i64,
    // Staff-only, for the customer detail view.
    pub note: Option<String>,
    pub tags: Vec<String>,

    // Marketing consent. Separate from the transactional notification
    // settings: unsubscribing from the newsletter must not stop the "your
    // order shipped" email.
    pub accepts_marketing: bool,
    pub marketing_updated_at: Option<i64>,

    pub created_at: i64,
    pub modified_at: i64,
}

/*
  notification_preferences — one row per user, all opt-out by default.

  Transactional topics (order, delivery, support, security) are separated from
  marketing ones because they are not the same promise: unsubscribing from the
  newsletter must never stop "your parcel is out for delivery", and in most
  jurisdictions it legally can't. Only `promotions` and `weekly_digest` belong
  to `CustomerProfile.accepts_marketing`.

  Absent row means "everything on except the digest" — see the handler's
  fallback. Storing a row only once someone changes something keeps the
  collection to the users who actually expressed a preference.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationPreferences {
    pub uuid: String,
    pub user_id: String,

    /* ── Transactional ──────────────────────────────────── */
    pub order_updates: bool,
    pub delivery_updates: bool,
    pub support_replies: bool,
    // Sign-in from a new device, password change. Worth its own flag so it can
    // be forced on for everyone if that policy ever changes.
    pub security_alerts: bool,

    /* ── Catalogue watches ──────────────────────────────── */
    pub price_drops: bool,
    pub back_in_stock: bool,
    pub review_reminders: bool,

    /* ── Marketing ──────────────────────────────────────── */
    pub promotions: bool,
    pub weekly_digest: bool,

    /* ── Channels ───────────────────────────────────────── */
    // Which pipes the topics above are allowed to go down. A topic is sent
    // only where both its own flag and the channel are on.
    pub push_enabled: bool,
    pub email_enabled: bool,
    pub sms_enabled: bool,

    pub modified_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoyaltyReason {
    Purchase,
    Review,
    Referral,
    Redemption,
    Expiry,
    Adjustment,
}
impl std::fmt::Display for LoyaltyReason {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  loyalty_ledger — append-only, and the real answer to "how many points".

  `CustomerProfile.loyalty_points` is a cache of the sum. Points are close
  enough to money that "you had 240 and now you have 0" needs to be
  answerable — a bare counter cannot answer it.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoyaltyLedgerEntry {
    pub uuid: String,
    pub user_id: String,
    // Signed: earned is positive, spent is negative.
    pub points: i64,
    pub balance_after: i64,
    pub reason: LoyaltyReason,
    pub order_id: Option<String>,
    pub note: Option<String>,
    // Points that lapse if unused. None never expires.
    pub expires_at: Option<i64>,
    pub created_at: i64,
}
