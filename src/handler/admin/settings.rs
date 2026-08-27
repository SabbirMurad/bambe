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

/*
  The one way to read the store's settings.

  This existed as two ways, which is exactly the split the constant above
  was written to prevent: the console read the row by its fixed id, while
  checkout and the ticket numbering read `find_one({})` — whichever row
  Mongo happened to hand back. A shop seeded with any other uuid therefore
  charged 8% tax at checkout while the settings screen showed 0%, and
  saving from that screen wrote to a *second* document nothing else read.

  So this adopts a legacy row into the canonical id on first read rather
  than inserting a duplicate beside it, and everything now calls it.
*/
pub async fn load(db: &mongodb::Database) -> Option<crate::model::Store::StoreSettings> {
    use mongodb::bson::doc;
    let col = db.collection::<crate::model::Store::StoreSettings>("store_settings");

    if let Ok(Some(settings)) = col.find_one(doc! { "uuid": STORE_SETTINGS_ID }).await {
        return Some(settings);
    }

    // No canonical row. If the shop has one under another id, that is the
    // real one — take it over rather than starting a second.
    let legacy = col.find_one(doc! {}).await.ok().flatten()?;
    col.update_one(
        doc! { "uuid": &legacy.uuid },
        doc! { "$set": { "uuid": STORE_SETTINGS_ID } },
    )
    .await
    .ok()?;

    col.find_one(doc! { "uuid": STORE_SETTINGS_ID }).await.ok().flatten()
}
