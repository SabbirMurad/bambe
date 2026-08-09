use serde::{Deserialize, Serialize};
use super::Cents;

/*
  Publication state, set by an admin.

  The products table also shows "Low stock" and "Out of stock", but those are
  not stored here: they are read off the variants. Stock changes on every sale,
  and a status field would have to be rewritten by the checkout to stay true —
  two sources for one fact, drifting the first time a write fails.
*/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ProductStatus {
    Active,
    #[default]
    Draft,
    Archived,
}
impl std::fmt::Display for ProductStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

//product
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Product {
    pub uuid: String,
    pub title: String,
    // URL key, unique. Kept beside the title so renaming a product for display
    // doesn't silently break every link that already points at it.
    pub slug: String,
    pub description: String,
    pub brand_id: Option<String>,
    pub category_id: String,
    pub status: ProductStatus,

    /*
      Denormalised from the variants. A listing page sorts and filters on price
      across hundreds of products at once; without these it would have to load
      every variant of every product to know what to show. Recomputed whenever
      a variant is written.
    */
    pub price_min: Cents,
    pub price_max: Cents,
    pub in_stock: bool,
    // The struck-through "was" price behind the discount badge. None when the
    // product is not on offer.
    pub compare_at_price: Option<Cents>,

    // Aggregates over product_review and order_item respectively — the rating
    // score, review count and "2.4k sold" on the product page.
    pub rating_average: f32,
    pub rating_count: u32,
    pub sold_count: u32,

    pub created_at: i64,
    pub created_by: String,
    pub modified_at: i64,
    // Soft delete: an archived product still has to render inside old orders.
    pub deleted_at: Option<i64>,
}

/*
  product_tag — one row per tag per product.

  A tag on the product would only ever be searched from the product's side:
  finding every product tagged "summer" means scanning the catalogue and
  testing each array. As rows, an index on `slug` answers it directly, and the
  same index gives the tag cloud its counts without touching a product.

  Unique on (product_id, slug).
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProductTag {
    pub uuid: String,
    pub product_id: String,
    // What the admin typed — "Summer Sale". Shown on the product page.
    pub name: String,
    /*
      The normalised form the lookup actually uses — "summer-sale". Kept
      beside the display name because tags are free text: "Summer Sale",
      "summer sale" and "SUMMER SALE" are one tag to a shopper, and matching on
      `name` would file them as three.
    */
    pub slug: String,
    pub created_at: i64,
}

/*
  product_variant — the sellable unit.

  Price and stock live here, not on the product: the admin's variant table
  gives every size/colour row its own price, stock and SKU, and a cart line has
  to point at exactly one of them.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProductVariant {
    pub uuid: String,
    pub product_id: String,
    pub sku: String,

    pub size: Option<String>,
    pub color: Option<String>,
    // The swatch shown on the product page, e.g. "#1A1A18".
    pub color_hex: Option<String>,
    /*
      The variant's photograph — an ImageStruct uuid. Photographs are of a
      variant, not of a product: the black jacket and the tan one are the same
      product and do not share a picture, which is why it is uploaded on the
      variant row in the admin form.

      Option because a draft can have its sizes and prices filled in before
      the photography comes back.
    */
    pub image_id: Option<String>,

    pub price: Cents,
    pub compare_at_price: Option<Cents>,

    pub stock: i32,
    // Per-variant, because "low" is not one number for the whole shop: five
    // left is a crisis for a bestseller and normal for a display cabinet.
    pub low_stock_threshold: i32,

    pub created_at: i64,
    pub modified_at: i64,
}

//brand
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Brand {
    pub uuid: String,
    pub name: String,
    pub slug: String,
    pub logo_image_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StockReason {
    Restock,
    Sale,
    Return,
    Cancellation,
    Adjustment,
    Damage,
}
impl std::fmt::Display for StockReason {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  stock_movement — append-only ledger behind ProductVariant.stock.

  The counter alone can only ever answer "how many now". When it comes out
  wrong — and oversells are the bug you will actually get — this is what says
  which sale, return or manual edit moved it, and by how much.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StockMovement {
    pub uuid: String,
    pub variant_id: String,
    // Signed: -1 for a sale, +1 for a return.
    pub delta: i32,
    // Level after applying `delta`, so a replay can be checked against the
    // counter without re-summing the whole ledger.
    pub balance_after: i32,
    pub reason: StockReason,
    pub order_id: Option<String>,
    pub note: Option<String>,
    // None when the system moved it (a sale); set for a manual adjustment.
    pub created_by: Option<String>,
    pub created_at: i64,
}
