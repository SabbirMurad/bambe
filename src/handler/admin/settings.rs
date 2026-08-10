pub mod get_store;
pub use get_store as GetStore;

pub mod update_store;
pub use update_store as UpdateStore;

pub mod shipping_zone;
pub use shipping_zone as ShippingZone;

pub mod staff;
pub use staff as Staff;

pub mod audit_log;
pub use audit_log as AuditLog;

// Fixed id for the one StoreSettings document — reading "the first one you
// find" would silently split the shop's identity if a second row ever
// appeared, per the model's own comment.
pub const STORE_SETTINGS_ID: &str = "store-settings";
