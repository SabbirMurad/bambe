use serde::{Deserialize, Serialize};
use super::{Cents, CurrencyCode};

/*
  store_settings — a single document. Read it by a fixed uuid rather than
  "the first one you find": a second row here silently splits the shop's
  identity in two.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoreSettings {
    pub uuid: String,

    pub store_name: String,
    pub support_email: String,
    pub support_phone: Option<String>,
    pub business_address: String,
    pub default_currency: CurrencyCode,
    // IANA name ("America/New_York"). Delivery cutoffs and "ends today" on a
    // coupon are meaningless without the shop's own clock.
    pub timezone: String,

    pub logo_image_id: Option<String>,
    // Prefixes for the human-readable references — "SE" gives SE-2026-78432,
    // "TK" gives TK-2035.
    pub order_number_prefix: String,
    pub ticket_number_prefix: String,

    // Applied when no zone rule matches. Basis points: 875 = 8.75%.
    pub tax_rate_bps: u32,
    pub prices_include_tax: bool,

    // Takes the storefront down without stopping the server, so staff can
    // still reach the admin console.
    pub maintenance_mode: bool,

    // The storefront's colour theme, chosen in Admin → Settings → Store:
    // "orange" (the default, also what None means), "crimson" or "green". Written
    // onto every page's <html> by src/markup.rs.
    #[serde(default)]
    pub theme: Option<String>,

    pub modified_at: i64,
    pub modified_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShippingRateKind {
    Flat,
    Free,
    // Worked out at checkout from weight or a carrier API.
    Calculated,
}
impl std::fmt::Display for ShippingRateKind {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  shipping_zone — a set of countries and what shipping costs to them.

  Zones are matched most-specific-first, so a "Domestic" zone listing one
  country wins over a "Rest of world" catch-all. Give the catch-all an empty
  `countries` and match it last.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShippingZone {
    pub uuid: String,
    pub name: String,
    // ISO-3166 alpha-2 codes. Empty means "everywhere not matched above".
    pub countries: Vec<String>,
    pub rate_kind: ShippingRateKind,
    pub rate: Cents,
    // Waives the rate over this subtotal — the "free over $50" row.
    pub free_above: Option<Cents>,
    pub enabled: bool,
    pub position: u16,
    pub modified_at: i64,
}

/*
  What a staff account may do in the admin console.

  Roles rather than per-screen permission flags: four fixed roles cover the
  whole console today, and a flag set that nobody has asked for is a schema to
  maintain for a feature that may never exist.
*/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StaffRole {
    Owner,
    Manager,
    SupportAgent,
    // Read-only — reports and lists, no writes.
    Analyst,
}
impl std::fmt::Display for StaffRole {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum StaffStatus {
    // Invited, hasn't signed in yet.
    #[default]
    Pending,
    Active,
    Suspended,
}
impl std::fmt::Display for StaffStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  staff_member — the console-side profile of an admin account.

  The credentials stay in `admin_account`; this holds the role and the activity
  the Staff table shows. Keeping them apart means changing someone's role never
  touches the record their password hash lives in.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaffMember {
    pub uuid: String,
    // AdminAccount.uuid
    pub admin_id: String,
    pub name: String,
    pub email: String,
    pub role: StaffRole,
    pub status: StaffStatus,
    pub avatar_image_id: Option<String>,

    pub last_active_at: Option<i64>,
    pub invited_by: Option<String>,
    pub invited_at: i64,
    pub joined_at: Option<i64>,
}

/*
  admin_audit_log — who changed what in the console.

  Anything that moves money or stock needs an author and a timestamp that the
  changed document itself doesn't carry: a product row records `modified_by`,
  but not that the price went from $44.99 to $29.99, and that is the question
  actually asked after a bad day of trading.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdminAuditLog {
    pub uuid: String,
    pub admin_id: String,
    pub admin_name: String,
    // "product.price_changed", "order.refunded", "coupon.disabled"
    pub action: String,
    pub target_collection: String,
    pub target_id: String,
    // Only the fields that moved, as JSON — a full before/after copy of a
    // large document buries the change it is meant to show.
    pub changes: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: i64,
}
