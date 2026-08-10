use serde::{Deserialize, Serialize};

/*
  cart — one open cart per user.

  Guests get one too, keyed by `session_id` with `user_id` empty; signing in
  merges that cart into the account's. Storing the cart server-side (rather
  than in localStorage) is what lets the phone and the laptop show the same
  basket, and what lets the shop see what people abandon.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cart {
    pub uuid: String,
    pub user_id: Option<String>,
    // Set for guests, cleared once the cart is claimed by an account.
    pub session_id: Option<String>,

    // Applied at the cart, validated again at payment — a code can expire or
    // hit its limit between the two.
    pub coupon_code: Option<String>,

    pub created_at: i64,
    pub modified_at: i64,
    // Set when the cart becomes an order. A converted cart is kept, not
    // deleted, so abandonment stats have a denominator.
    pub converted_order_id: Option<String>,
}

/*
  A line in the cart — its own collection, one row per line, joined back to
  the cart by `cart_id`.

  It holds only the variant and a quantity — no title, no price. Everything
  else is read live from the catalogue, so an item sitting in a cart for a week
  shows this week's price, and a variant that goes out of stock is caught at
  render rather than at payment. Prices are frozen later, in the order.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CartItem {
    pub uuid: String,
    pub cart_id: String,
    pub product_id: String,
    pub variant_id: String,
    pub quantity: u32,
    pub added_at: i64,
    /*
      The price when it was added. Not used to charge — it exists so the cart
      can tell the customer "this went up since you added it", which is the
      one thing a live price alone cannot say.
    */
    pub price_when_added: super::Cents,
}

//wishlist_item
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WishlistItem {
    pub uuid: String,
    pub user_id: String,
    pub product_id: String,
    /*
      Optional, unlike in the cart: a wishlist is usually "I want this jacket",
      not "I want this jacket in M". Set when they picked a size, so the price
      drop and back-in-stock alerts can watch the right variant.
    */
    pub variant_id: Option<String>,
    // Price at the moment of saving — the baseline a "price dropped" alert
    // compares against.
    pub price_when_added: super::Cents,
    pub notify_on_restock: bool,
    pub notify_on_price_drop: bool,
    pub created_at: i64,
}

/*
  recently_viewed — one row per user per product, upserted on view.

  Capped per user in the write path (keep the newest N), because this grows
  faster than anything else in the system and nobody browses their own tail.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecentlyViewed {
    pub uuid: String,
    pub user_id: String,
    pub product_id: String,
    pub viewed_at: i64,
}
