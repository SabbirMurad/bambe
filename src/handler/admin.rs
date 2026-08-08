use mongodb::bson::doc;
use mongodb::Database;
use serde_json::{json, Value};
use crate::Model::Account::AccountCore;

pub mod deletion_requests;
pub use deletion_requests as DeletionRequests;

pub mod admins;
pub use admins as Admins;

pub mod me;
pub use me as Me;

/// Resolve a user id to `{ user_id, name }` for display in the console.
pub async fn display_name(db: &Database, user_id: &str) -> Value {
    let name = db
        .collection::<AccountCore>("account_core")
        .find_one(doc!{ "uuid": user_id })
        .await
        .ok()
        .flatten()
        .map(|a| a.username)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Unknown".to_string());

    json!({ "user_id": user_id, "name": name })
}
