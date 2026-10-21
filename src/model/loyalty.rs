use serde::{Deserialize, Serialize};

/// Stamps on a full card.
pub const CARD_SIZE: usize = 10;
/// The stamp at which staff are asked to line up the reward.
pub const ALMOST_THERE: usize = CARD_SIZE - 1;

/*
  loyalty_card — the stamp card a customer fills one delivered order at a
  time.

  The stamps are the delivered order ids themselves, not a counter, so an
  order can only ever stamp once (re-saving "Delivered" is a no-op) and
  "which orders earned this card" stays answerable.

  The reward is a personal coupon staff attach to the card, at any point
  before or after it fills. It stays disabled until the customer redeems the
  full card, which is when it gets its validity window.

  A full card that hasn't been redeemed yet doesn't block the next one: the
  following delivery starts card number + 1.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoyaltyCard {
    pub uuid: String,
    pub user_id: String,
    // 1 for a customer's first card, 2 for the next…
    pub number: u32,
    pub order_ids: Vec<String>,

    pub reward_coupon_id: Option<String>,
    // How long the coupon stays usable once redeemed. None never expires.
    pub reward_valid_days: Option<u32>,
    // Staff's note to the customer, shown with the reward.
    #[serde(default)]
    pub reward_message: Option<String>,

    // How many stamps the customer has already watched land, so the
    // animation plays only for the new ones.
    pub seen_stamps: u32,

    pub completed_at: Option<i64>,
    pub redeemed_at: Option<i64>,
    pub created_at: i64,
    pub modified_at: i64,
}

impl LoyaltyCard {
    pub fn stamps(&self) -> usize { self.order_ids.len().min(CARD_SIZE) }
    pub fn is_full(&self) -> bool { self.order_ids.len() >= CARD_SIZE }
}
