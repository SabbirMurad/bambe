pub mod list;
pub use list as List;

pub mod moderate;
pub use moderate as Moderate;

pub mod reply;
pub use reply as Reply;

pub mod remove;
pub use remove as Remove;

use futures::StreamExt;
use mongodb::Database;
use mongodb::bson::{doc, Document};
use crate::model::Review::ProductReview;

/*
  A whole-order review is one row per product sharing a `group_id`, but to
  the customer it is one review — so hiding, replying to or deleting any
  row acts on all of them. Returns the filter that selects them and the
  products they sit on, or None when the review doesn't exist.
*/
pub async fn rows_of(db: &Database, review_id: &str) -> Option<(Document, Vec<ProductReview>)> {
    let col = db.collection::<ProductReview>("product_review");
    let review = col.find_one(doc! { "uuid": review_id }).await.ok().flatten()?;

    let filter = match &review.group_id {
        Some(group) => doc! { "group_id": group },
        None => doc! { "uuid": &review.uuid },
    };

    let mut rows = Vec::new();
    if let Ok(mut cursor) = col.find(filter.clone()).await {
        while let Some(result) = cursor.next().await {
            match result {
                Ok(r) => rows.push(r),
                Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
            }
        }
    }
    Some((filter, rows))
}
