use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ReviewStatus {
    #[default]
    Pending,
    Published,
    Rejected,
}
impl std::fmt::Display for ReviewStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

//product_review
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProductReview {
    pub uuid: String,
    pub product_id: String,
    pub user_id: String,
    /*
      Which order this review came from. Set when the reviewer actually bought
      the item, and it is what backs the "verified purchase" badge — a boolean
      alone can be set by anything, an order id can be checked.
    */
    pub order_id: Option<String>,
    // The variant they bought, so the review can say "Black · XL".
    pub variant_id: Option<String>,

    // 1..=5. Validate on write; the aggregates on the product assume it.
    pub rating: u8,
    pub title: Option<String>,
    pub body: String,
    pub image_ids: Vec<String>,

    pub status: ReviewStatus,
    pub rejection_reason: Option<String>,

    // Denormalised from review_vote for sorting by "most helpful".
    pub helpful_count: u32,
    // A single reply from the shop, shown under the review.
    pub reply: Option<ReviewReply>,

    pub created_at: i64,
    pub modified_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewReply {
    pub body: String,
    pub replied_by: String,
    pub replied_at: i64,
}

/*
  review_vote — one row per user per review.

  A row rather than a counter increment, so a second click can toggle the vote
  off instead of inflating the total, and so one account cannot mark the same
  review helpful fifty times.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewVote {
    pub uuid: String,
    pub review_id: String,
    pub user_id: String,
    pub helpful: bool,
    pub created_at: i64,
}
