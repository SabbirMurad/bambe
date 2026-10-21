use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdminNotifKind {
    // A customer is one stamp from a full card — time to pick their reward.
    LoyaltyAlmostThere,
    // A card filled up with no reward attached; the customer is waiting.
    LoyaltyRewardMissing,
}

/*
  admin_notification — the console's own feed, behind the navbar bell.

  Separate from `notification`, which is keyed on a shopper's account: staff
  live in `admin_account`. One shared feed for the whole team — whoever acts
  on it marks it read for everyone, which is what a to-do wants.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdminNotification {
    pub uuid: String,
    pub kind: AdminNotifKind,
    pub title: String,
    pub body: String,
    // Console path to open, e.g. "/admin/customers?customer=<id>".
    pub link: Option<String>,
    // The customer it is about.
    pub user_id: Option<String>,
    pub read_at: Option<i64>,
    pub created_at: i64,
}
