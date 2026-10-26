use std::collections::HashMap;
use futures::StreamExt;
use mongodb::bson::{doc, Document};
use mongodb::Database;

pub mod list;
pub use list as List;

pub mod create;
pub use create as Create;

pub mod update;
pub use update as Update;

pub mod remove;
pub use remove as Remove;

/*
  The brand's URL key, made from its name: "The Ordinary" -> "the-ordinary".
  The admin never types one; a brand is chosen from a list, not linked by
  hand, so the name is the only thing they should have to get right.
*/
pub fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in input.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/*
  Live products per brand, for the admin list and the delete guard. Counted
  on demand rather than stored: a brand's count would have to be rewritten
  by every product create, edit and delete to stay true, and the brand list
  is small enough to count in one aggregate.
*/
pub async fn product_counts(db: &Database) -> HashMap<String, u32> {
    let pipeline = vec![
        doc! { "$match": { "deleted_at": null, "brand_id": { "$ne": null } } },
        doc! { "$group": { "_id": "$brand_id", "count": { "$sum": 1 } } },
    ];
    let mut counts = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Document>("product").aggregate(pipeline).await {
        while let Some(result) = cursor.next().await {
            let d = match result {
                Ok(d) => d,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            if let (Ok(id), Ok(count)) = (d.get_str("_id"), d.get_i32("count")) {
                counts.insert(id.to_string(), count as u32);
            }
        }
    }
    counts
}
