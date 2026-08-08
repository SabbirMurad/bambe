use serde::{Deserialize, Serialize};

// snake_case so the multi-word variants read as "price_drop" rather than
// "pricedrop".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifType {
    /* ── Commerce ───────────────────────────────────────── */
    OrderPlaced,
    OrderConfirmed,
    OrderShipped,
    OrderDelivered,
    OrderCancelled,
    RefundIssued,
    // A wishlisted item came back, or got cheaper.
    BackInStock,
    PriceDrop,
    // Sales and campaigns — the only type gated on marketing consent.
    Promotion,
    LoyaltyPoints,
    ReviewReminder,
    SupportReply,
    // New-device sign-in, password change.
    Security,
    System,
}

//notification
#[derive(Debug, Serialize, Deserialize)]
pub struct AppNotification {
    pub uuid: String,
    pub recipient_id: String,
    pub r#type: NotifType,
    pub title: String,
    pub body: String,

    // The thumbnail in the notification list — a product shot, an order's
    // first item.
    pub image_id: Option<String>,
    /*
      What tapping it opens: an order id, a product id, a ticket id. Read
      against `r#type`, which is what says which collection it points into —
      one nullable id beats a column per target.
    */
    pub related_id: Option<String>,
    // Deep link path, e.g. "/account/orders". Stored rather than derived so an
    // old notification keeps working after the routes move.
    pub link: Option<String>,

    pub is_read: bool,
    pub created_at: i64,
}
