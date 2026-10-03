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

pub mod product;
pub use product as Product;

pub mod category;
pub use category as Category;

pub mod order;
pub use order as Order;

pub mod return_request;
pub use return_request as ReturnRequest;

pub mod coupon;
pub use coupon as Coupon;

pub mod customer;
pub use customer as Customer;

pub mod support_ticket;
pub use support_ticket as SupportTicket;

pub mod settings;
pub use settings as Settings;

pub mod dashboard;
pub use dashboard as Dashboard;

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
