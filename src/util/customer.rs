use uuid::Uuid;
use mongodb::bson::doc;
use mongodb::Database;
use crate::model::Cents;
use crate::model::Customer::{CustomerProfile, CustomerSegment};

// Placeholder tiers until the store defines its own — nothing in
// `StoreSettings` configures these yet, so a reasonable default keeps the
// segment field meaningful rather than leaving it permanently "New".
const VIP_LIFETIME_SPEND: Cents = 100_000; // $1,000
const DORMANT_AFTER_MS: i64 = 180 * 24 * 60 * 60 * 1000; // 180 days

pub fn derive_segment(order_count: u32, lifetime_spend: Cents, last_order_at: Option<i64>, now: i64) -> CustomerSegment {
    if lifetime_spend >= VIP_LIFETIME_SPEND {
        return CustomerSegment::Vip;
    }
    if let Some(last) = last_order_at {
        if now - last > DORMANT_AFTER_MS {
            return CustomerSegment::Dormant;
        }
    }
    if order_count > 1 {
        CustomerSegment::Repeat
    } else {
        CustomerSegment::New
    }
}

/*
  Rolls a just-placed order into the customer's commerce profile — the
  rewrite-on-every-purchase half the model comment describes. Called from
  checkout rather than derived on read, because "lifetime spend" has to
  match the sum of actual orders even after this purchase is long past.
*/
pub async fn roll_up_purchase(db: &Database, user_id: &str, amount: Cents, now: i64) {
    let col = db.collection::<CustomerProfile>("customer_profile");
    let existing = col.find_one(doc! { "user_id": user_id }).await.ok().flatten();

    let order_count = existing.as_ref().map(|c| c.order_count + 1).unwrap_or(1);
    let lifetime_spend = existing.as_ref().map(|c| c.lifetime_spend + amount).unwrap_or(amount);
    let segment = derive_segment(order_count, lifetime_spend, Some(now), now);

    col.update_one(
        doc! { "user_id": user_id },
        doc! {
            "$set": {
                "order_count": order_count,
                "lifetime_spend": lifetime_spend,
                "last_order_at": now,
                "segment": mongodb::bson::to_bson(&segment).unwrap(),
                "modified_at": now,
            },
            "$setOnInsert": {
                "uuid": Uuid::now_v7().to_string(),
                "user_id": user_id,
                "loyalty_points": 0i64,
                "tags": Vec::<String>::new(),
                "accepts_marketing": false,
                "created_at": now,
            },
        },
    )
    .upsert(true)
    .await
    .ok();
}
