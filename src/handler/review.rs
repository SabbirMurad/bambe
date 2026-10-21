pub mod list;
pub use list as List;

pub mod create;
pub use create as Create;

pub mod vote;
pub use vote as Vote;

pub mod eligible;
pub use eligible as Eligible;

pub mod reviewed;
pub use reviewed as Reviewed;

use futures::StreamExt;
use mongodb::Database;
use mongodb::bson::{doc, Document};
use crate::model::Product::Product;

/*
  Recomputes a product's stored average and count from its published
  reviews. Recomputed rather than nudged, so a missed update can never
  leave the number drifting from the reviews underneath it.
*/
pub async fn refresh_rating(db: &Database, product_id: &str) {
    let pipeline = vec![
        doc! { "$match": { "product_id": product_id, "status": "Published", "deleted_at": null } },
        doc! { "$group": { "_id": null, "avg": { "$avg": "$rating" }, "count": { "$sum": 1 } } },
    ];
    let (average, count) = match db.collection::<Document>("product_review").aggregate(pipeline).await {
        Ok(mut cursor) => match cursor.next().await {
            Some(Ok(d)) => (d.get_f64("avg").unwrap_or(0.0), d.get_i32("count").unwrap_or(0)),
            _ => (0.0, 0),
        },
        Err(error) => {
            log::error!("{:?}", error);
            return;
        }
    };

    // One decimal place is all the storefront ever shows.
    let average = (average * 10.0).round() / 10.0;
    if let Err(error) = db
        .collection::<Product>("product")
        .update_one(
            doc! { "uuid": product_id },
            doc! { "$set": { "rating_average": average, "rating_count": count } },
        )
        .await
    {
        log::error!("{:?}", error);
    }
}
