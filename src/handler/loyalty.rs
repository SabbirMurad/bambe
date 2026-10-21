pub mod cards;
pub use cards as Cards;

pub mod seen;
pub use seen as Seen;

pub mod redeem;
pub use redeem as Redeem;

use serde_json::{json, Value};
use mongodb::Database;
use mongodb::bson::doc;
use crate::model::Coupon::{Coupon, DiscountType};
use crate::model::Loyalty::{LoyaltyCard, CARD_SIZE};

fn dollars(cents: i64) -> String {
    if cents % 100 == 0 { format!("${}", cents / 100) } else { format!("${}.{:02}", cents / 100, cents % 100) }
}

/// "20% off (up to $15) on orders over $40" — the reward in one line.
pub fn describe(coupon: &Coupon) -> String {
    let mut text = match coupon.discount_type {
        DiscountType::Percentage => {
            let pct = coupon.value as f64 / 100.0;
            if pct.fract() == 0.0 { format!("{}% off", pct as i64) } else { format!("{:.1}% off", pct) }
        }
        DiscountType::Fixed => format!("{} off", dollars(coupon.value)),
        DiscountType::FreeShipping => "Free shipping".to_string(),
    };
    if let Some(max) = coupon.max_discount {
        if coupon.discount_type == DiscountType::Percentage { text.push_str(&format!(" (up to {})", dollars(max))); }
    }
    if let Some(min) = coupon.min_spend {
        text.push_str(&format!(" on orders over {}", dollars(min)));
    }
    text
}

/// A card as the customer sees it. The code is only in here once the card
/// has been redeemed — before that it is a surprise, and an unusable one.
/// `staff` shows the reward and its code at every stage, for the console.
pub async fn card_view(db: &Database, card: &LoyaltyCard, staff: bool) -> Value {
    let coupon = match &card.reward_coupon_id {
        Some(id) => db.collection::<Coupon>("coupon").find_one(doc! { "uuid": id }).await.ok().flatten(),
        None => None,
    };
    let now = chrono::Utc::now().timestamp_millis();

    let status = if card.redeemed_at.is_some() {
        "redeemed"
    } else if card.is_full() {
        "full"
    } else {
        "collecting"
    };

    let reward = coupon.as_ref().map(|c| {
        let redeemed = card.redeemed_at.is_some();
        json!({
            "summary": describe(c),
            "description": card.reward_message,
            "code": if redeemed || staff { Some(c.code.clone()) } else { None },
            "valid_until": if redeemed { c.ends_at } else { None },
            "valid_days": card.reward_valid_days,
            "used": c.used_count > 0,
            "expired": redeemed && c.ends_at.map(|e| e < now).unwrap_or(false),
            // The raw terms, for the console's edit form.
            "terms": if staff { json!({
                "discount_type": c.discount_type,
                "value": c.value,
                "max_discount": c.max_discount,
                "min_spend": c.min_spend,
            }) } else { Value::Null },
        })
    });

    json!({
        "uuid": card.uuid,
        "number": card.number,
        "size": CARD_SIZE,
        "stamps": card.stamps(),
        "seen_stamps": card.seen_stamps,
        "status": status,
        // Only whether one is ready, before the card fills — the details
        // are part of the reveal.
        "reward_ready": coupon.is_some(),
        "reward": if status == "collecting" && !staff { Value::Null } else { reward.unwrap_or(Value::Null) },
        "completed_at": card.completed_at,
        "redeemed_at": card.redeemed_at,
    })
}
