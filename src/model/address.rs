use serde::{Deserialize, Serialize};

/*
  address — an entry in the customer's address book.

  Fields match the checkout form one-for-one. `state` and `zip` are optional
  because plenty of countries have neither, and a required field there blocks
  a real customer from checking out.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Address {
    pub uuid: String,
    pub user_id: String,

    // What the customer calls it — "Home", "Office". Not a fixed enum: the
    // form lets them type anything.
    pub label: String,
    pub recipient_name: String,
    pub street: String,
    pub city: String,
    pub state: Option<String>,
    pub zip: Option<String>,
    // ISO-3166 alpha-2 ("US"), not the display name — shipping rates and tax
    // rules key off the code, and display names change with locale.
    pub country: String,
    pub phone: String,
    // Delivery instructions, e.g. "leave with the concierge".
    pub note: Option<String>,

    /*
      At most one default per user. Enforce it in the write path — set the new
      one and clear the rest in a transaction — because a partial write here
      leaves the checkout with two defaults and no way to choose.
    */
    pub is_default: bool,

    pub created_at: i64,
    pub modified_at: i64,
    // Soft delete: an order placed to this address keeps its own snapshot, but
    // the row is still worth keeping for the customer's own history.
    pub deleted_at: Option<i64>,
}

/*
  A frozen copy of an address as it read when an order was placed, embedded in
  the order document.

  This is deliberately not a reference to `Address`. A customer who moves and
  edits their address book must not retroactively change where last year's
  parcel was sent — and deleting an address must not blank out an old invoice.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddressSnapshot {
    // Where it came from, for "reuse this address". May no longer exist.
    pub address_id: Option<String>,
    pub label: String,
    pub recipient_name: String,
    pub street: String,
    pub city: String,
    pub state: Option<String>,
    pub zip: Option<String>,
    pub country: String,
    pub phone: String,
    pub note: Option<String>,
}
