//! Index creation for the storefront collections.
//!
//! The uniqueness constraints below were documented in the model comments and
//! enforced nowhere, so nothing stopped two products sharing a slug or two
//! orders sharing a number. `create_index` is idempotent — re-running it with
//! the same spec is a no-op — so this is safe on every startup.
//!
//! Failures are logged, not fatal: an index that can't be built (usually
//! because existing data already violates it) should surface in the log
//! without taking the server down, and the duplicates get cleaned up by hand.

use mongodb::bson::doc;
use mongodb::{IndexModel, Database};
use mongodb::options::IndexOptions;

pub async fn ensure_indexes(db: &Database) {
    /*
      Bail out if the database isn't there.

      Every `create_index` against an unreachable server waits out the driver's
      full server-selection timeout, so the two dozen below turn a dead Mongo
      into a twelve-minute startup that looks like a hang. One cheap probe
      first: if it fails, say so once and skip the rest — the server still
      comes up, and the first request will report the real problem.
    */
    if let Err(error) = db.list_collection_names().await {
        log::warn!("skipping index creation, database unreachable: {:?}", error);
        return;
    }

    // ── uniqueness ────────────────────────────────────────────────────────
    /*
      Partial, not plain unique: a deleted product keeps its row (soft
      delete in admin/product/remove.rs), and a plain unique index would
      let it reserve its slug for ever — the title could never be used
      again. Scoping the index to live rows matches the check in
      admin/product/create.rs.
    */
    unique_where(
        db, "product", doc! { "slug": 1 },
        doc! { "deleted_at": null }, "product_slug_live",
    ).await;
    // The unscoped index this replaces. Dropped rather than left: it
    // would keep enforcing the old rule alongside the new one.
    drop_index(db, "product", "product_slug").await;
    unique(db, "order", doc! { "number": 1 }, "order_number").await;
    unique(db, "coupon", doc! { "code": 1 }, "coupon_code").await;
    // Two admins adding the same brand at once both pass create.rs's check.
    unique(db, "brand", doc! { "slug": 1 }, "brand_slug").await;
    unique(db, "newsletter_subscriber", doc! { "email_address": 1 }, "newsletter_email").await;
    unique(db, "counter", doc! { "uuid": 1 }, "counter_uuid").await;
    unique(
        db,
        "payment_transaction",
        doc! { "provider_transaction_id": 1 },
        "payment_provider_txn",
    ).await;
    // One redemption per coupon per order — what stops a retry double-counting.
    unique(
        db,
        "coupon_redemption",
        doc! { "coupon_id": 1, "order_id": 1 },
        "coupon_redemption_pair",
    ).await;

    // ── lookup paths ──────────────────────────────────────────────────────
    // Every one of these backs a filter that currently collection-scans.
    plain(db, "order", doc! { "user_id": 1, "placed_at": -1 }, "order_by_user").await;
    plain(db, "order_item", doc! { "order_id": 1 }, "order_item_by_order").await;
    plain(db, "order_item", doc! { "variant_id": 1 }, "order_item_by_variant").await;
    plain(db, "product_variant", doc! { "product_id": 1 }, "variant_by_product").await;
    plain(db, "product", doc! { "category_id": 1, "status": 1 }, "product_by_category").await;
    plain(db, "cart_item", doc! { "cart_id": 1 }, "cart_item_by_cart").await;
    plain(db, "cart", doc! { "user_id": 1 }, "cart_by_user").await;
    plain(db, "product_review", doc! { "product_id": 1, "created_at": -1 }, "review_by_product").await;
    // One review per product per order — the create handler checks first,
    // this stops two quick submits both getting through.
    unique(db, "product_review", doc! { "user_id": 1, "order_id": 1, "product_id": 1 }, "review_per_order_product").await;
    // One vote per user per review — the vote endpoint reads it back to toggle.
    unique(db, "review_vote", doc! { "review_id": 1, "user_id": 1 }, "review_vote_pair").await;
    plain(db, "notification", doc! { "user_id": 1, "created_at": -1 }, "notification_by_user").await;
    unique(db, "loyalty_card", doc! { "user_id": 1, "number": 1 }, "loyalty_card_number").await;
    plain(db, "loyalty_card", doc! { "order_ids": 1 }, "loyalty_card_by_order").await;
    plain(db, "admin_notification", doc! { "created_at": -1 }, "admin_notification_recent").await;
    plain(db, "return_request", doc! { "order_id": 1 }, "return_by_order").await;
    plain(db, "return_request", doc! { "user_id": 1, "requested_at": -1 }, "return_by_user").await;
    plain(db, "return_item", doc! { "return_request_id": 1 }, "return_item_by_request").await;
    plain(db, "support_ticket", doc! { "user_id": 1, "created_at": -1 }, "ticket_by_user").await;
    plain(db, "ticket_message", doc! { "ticket_id": 1, "created_at": 1 }, "message_by_ticket").await;
    plain(db, "stock_movement", doc! { "variant_id": 1, "created_at": -1 }, "movement_by_variant").await;
    plain(db, "payment_transaction", doc! { "order_id": 1 }, "txn_by_order").await;
    plain(db, "recently_viewed", doc! { "user_id": 1, "viewed_at": -1 }, "viewed_by_user").await;
    plain(db, "wishlist_item", doc! { "user_id": 1 }, "wishlist_by_user").await;
    plain(db, "admin_audit_log", doc! { "created_at": -1 }, "audit_recent").await;
}

async fn unique(db: &Database, collection: &str, keys: mongodb::bson::Document, name: &str) {
    build(db, collection, keys, name, true).await;
}

/*
  A unique index that only covers the rows matching `filter`. Used where
  a collection soft-deletes: uniqueness should hold among the live rows
  and ignore the tombstones.

  Note this is NOT sparse — `partialFilterExpression` and `sparse` are
  mutually exclusive in Mongo, and the filter already does the job.
*/
async fn unique_where(
    db: &Database,
    collection: &str,
    keys: mongodb::bson::Document,
    filter: mongodb::bson::Document,
    name: &str,
) {
    let options = IndexOptions::builder()
        .name(name.to_string())
        .unique(true)
        .partial_filter_expression(filter)
        .build();
    let model = IndexModel::builder().keys(keys).options(options).build();
    if let Err(error) = db.collection::<mongodb::bson::Document>(collection).create_index(model).await {
        log::warn!("{}.{} index: {:?}", collection, name, error);
    }
}

/*
  Drops an index by name, for one replaced by a differently-shaped
  version. `create_index` is only idempotent for an identical spec, so a
  superseded index has to go explicitly or it keeps enforcing the old
  rule. A missing index is not an error worth logging — after the first
  run that is the normal case.
*/
async fn drop_index(db: &Database, collection: &str, name: &str) {
    let _ = db
        .collection::<mongodb::bson::Document>(collection)
        .drop_index(name)
        .await;
}

async fn plain(db: &Database, collection: &str, keys: mongodb::bson::Document, name: &str) {
    build(db, collection, keys, name, false).await;
}

async fn build(db: &Database, collection: &str, keys: mongodb::bson::Document, name: &str, unique: bool) {
    // The builder is typestate-typed, so the two shapes can't share a variable.
    // Unique indexes are sparse, so the many documents that predate a field
    // don't all collide on a shared `null` and block the index outright.
    let options = if unique {
        IndexOptions::builder().name(name.to_string()).unique(true).sparse(true).build()
    } else {
        IndexOptions::builder().name(name.to_string()).build()
    };

    let model = IndexModel::builder().keys(keys).options(options).build();

    if let Err(error) = db.collection::<mongodb::bson::Document>(collection).create_index(model).await {
        log::warn!("{}.{} index: {:?}", collection, name, error);
    }
}
