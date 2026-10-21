//! Atomic, gap-free sequence numbers.
//!
//! Mongo has no auto-increment, so a counter lives as one document per key and
//! is advanced with a single `$inc` upsert. `find_one_and_update` returns the
//! post-increment document, which means every caller gets a distinct value even
//! under concurrency — unlike picking a random number and hoping.

use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Counter {
    pub uuid: String,
    pub seq: i64,
}

/// Advance `key` and return its new value. `None` only if the write failed, in
/// which case the caller has to refuse rather than invent a number.
pub async fn next(db: &mongodb::Database, key: &str) -> Option<i64> {
    db.collection::<Counter>("counter")
        .find_one_and_update(
            doc! { "uuid": key },
            doc! { "$inc": { "seq": 1 } },
        )
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await
        .ok()
        .flatten()
        .map(|c| c.seq)
}
