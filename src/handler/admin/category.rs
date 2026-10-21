use std::collections::HashMap;
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use mongodb::Database;
use crate::model::Category::Category;
use crate::model::Product::Product;

pub mod list;
pub use list as List;

pub mod create;
pub use create as Create;

pub mod update;
pub use update as Update;

pub mod remove;
pub use remove as Remove;

/*
  product_count is denormalised over a category *and* its descendants, so
  re-parenting or deleting a category can move the number on categories that
  weren't directly touched. Simplest correct fix is to recompute every
  category's count in one pass rather than trying to patch just the ones
  that changed.
*/
pub async fn recompute_counts(db: &Database) {
    let mut cat_cursor = db.collection::<Category>("category").find(doc! {}).await.expect("category lookup");
    let mut categories = Vec::new();
    while let Some(result) = cat_cursor.next().await {
        let c = match result {
            Ok(c) => c,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        categories.push(c);
    }

    let pipeline = vec![
        doc! { "$match": { "deleted_at": null } },
        doc! { "$group": { "_id": "$category_id", "count": { "$sum": 1 } } },
    ];
    let mut direct_counts: HashMap<String, u32> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Document>("product").aggregate(pipeline).await {
        while let Some(result) = cursor.next().await {
            let d = match result {
                Ok(d) => d,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            if let (Some(id), Some(count)) = (d.get_str("_id").ok(), d.get_i32("count").ok()) {
                direct_counts.insert(id.to_string(), count as u32);
            }
        }
    }

    fn descendants<'a>(id: &str, categories: &'a [Category], out: &mut Vec<&'a str>) {
        for c in categories.iter().filter(|c| c.parent_id.as_deref() == Some(id)) {
            out.push(c.uuid.as_str());
            descendants(&c.uuid, categories, out);
        }
    }

    for category in &categories {
        let mut ids = vec![category.uuid.as_str()];
        descendants(&category.uuid, &categories, &mut ids);
        let total: u32 = ids.iter().map(|id| direct_counts.get(*id).copied().unwrap_or(0)).sum();

        db.collection::<Category>("category")
            .update_one(doc! { "uuid": &category.uuid }, doc! { "$set": { "product_count": total } })
            .await
            .ok();
    }
}
