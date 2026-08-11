use futures::StreamExt;
use mongodb::bson::doc;
use mongodb::Database;
use crate::model::Product::{Product, ProductVariant};

pub mod list;
pub use list as List;

pub mod create;
pub use create as Create;

pub mod update;
pub use update as Update;

pub mod remove;
pub use remove as Remove;

pub mod add_variant;
pub use add_variant as AddVariant;

pub mod update_variant;
pub use update_variant as UpdateVariant;

pub mod remove_variant;
pub use remove_variant as RemoveVariant;

pub mod adjust_stock;
pub use adjust_stock as AdjustStock;

/*
  price_min/price_max/in_stock/cover_image_id on `product` are denormalised
  from its variants for the listing page's price and stock filters and its
  card thumbnail. Call this after any variant write (create, edit, delete,
  restock) so the product row never drifts from what its variants actually
  say.
*/
pub async fn recompute_aggregates(db: &Database, product_id: &str) {
    let mut cursor = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "product_id": product_id })
        .await
        .expect("variant lookup");
    let mut variants = Vec::new();
    while let Some(Ok(v)) = cursor.next().await {
        variants.push(v);
    }

    let price_min = variants.iter().map(|v| v.price).min().unwrap_or(0);
    let price_max = variants.iter().map(|v| v.price).max().unwrap_or(0);
    let in_stock = variants.iter().any(|v| v.stock > 0);
    // The oldest variant is the cover, so the thumbnail stays put as later
    // colourways are added — sorted here rather than trusted from the cursor,
    // which has no order of its own.
    let cover_image_id = variants
        .iter()
        .min_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.uuid.cmp(&b.uuid)))
        .map(|v| v.image_id.clone());

    db.collection::<Product>("product")
        .update_one(
            doc! { "uuid": product_id },
            doc! {
                "$set": {
                    "price_min": price_min,
                    "price_max": price_max,
                    "in_stock": in_stock,
                    "cover_image_id": cover_image_id,
                },
            },
        )
        .await
        .ok();
}
