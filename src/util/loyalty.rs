//! The stamp card: one stamp per delivered order, ten to a card, and a
//! staff-chosen coupon as the reward for a full one.
//!
//! Best-effort like `notify`: nothing here may fail the order status change
//! that triggered it, so errors are logged rather than returned.

use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use mongodb::Database;
use mongodb::bson::doc;
use crate::model::AdminNotification::{AdminNotification, AdminNotifKind};
use crate::model::Customer::{CustomerProfile, LoyaltyLedgerEntry, LoyaltyReason};
use crate::model::Loyalty::{LoyaltyCard, ALMOST_THERE, CARD_SIZE};
use crate::model::Notification::NotifType;
use crate::Model::Account::AccountCore;
use crate::util::notify::{send, Notice};

const CARDS: &str = "loyalty_card";

/// The card the next stamp goes on: the newest one that isn't full,
/// created if there isn't one.
pub async fn open_card(db: &Database, user_id: &str) -> Option<LoyaltyCard> {
    let col = db.collection::<LoyaltyCard>(CARDS);
    let latest = col
        .find_one(doc! { "user_id": user_id })
        .sort(doc! { "number": -1 })
        .await
        .ok()
        .flatten();

    if let Some(card) = &latest {
        if !card.is_full() && card.redeemed_at.is_none() {
            return latest;
        }
    }

    let now = Utc::now().timestamp_millis();
    let card = LoyaltyCard {
        uuid: Uuid::now_v7().to_string(),
        user_id: user_id.to_string(),
        number: latest.map(|c| c.number + 1).unwrap_or(1),
        order_ids: Vec::new(),
        reward_coupon_id: None,
        reward_valid_days: None,
        reward_message: None,
        seen_stamps: 0,
        completed_at: None,
        redeemed_at: None,
        created_at: now,
        modified_at: now,
    };
    match col.insert_one(&card).await {
        Ok(_) => Some(card),
        Err(error) => {
            log::error!("loyalty card for {}: {:?}", user_id, error);
            None
        }
    }
}

/// The card a reward set now would pay out on: the earliest one not yet
/// redeemed — a full card waiting on its reward comes before the one
/// still filling.
pub async fn reward_card(db: &Database, user_id: &str) -> Option<LoyaltyCard> {
    let waiting = db
        .collection::<LoyaltyCard>(CARDS)
        .find_one(doc! { "user_id": user_id, "redeemed_at": null })
        .sort(doc! { "number": 1 })
        .await
        .ok()
        .flatten();
    match waiting {
        Some(card) => Some(card),
        None => open_card(db, user_id).await,
    }
}

async fn customer_name(db: &Database, user_id: &str) -> String {
    db.collection::<AccountCore>("account_core")
        .find_one(doc! { "uuid": user_id })
        .await
        .ok()
        .flatten()
        .map(|a| a.username)
        .unwrap_or_else(|| "A customer".to_string())
}

pub async fn notify_admins(db: &Database, kind: AdminNotifKind, user_id: &str, title: String, body: String) {
    let note = AdminNotification {
        uuid: Uuid::now_v7().to_string(),
        kind,
        title,
        body,
        link: Some(format!("/admin/customers?customer={}", user_id)),
        user_id: Some(user_id.to_string()),
        read_at: None,
        created_at: Utc::now().timestamp_millis(),
    };
    if let Err(error) = db.collection::<AdminNotification>("admin_notification").insert_one(&note).await {
        log::error!("admin notification: {:?}", error);
    }
}

/// Marks the loyalty alerts about one customer as dealt with — called once
/// their reward is in place.
pub async fn resolve_admin_alerts(db: &Database, user_id: &str) {
    db.collection::<AdminNotification>("admin_notification")
        .update_many(
            doc! { "user_id": user_id, "read_at": null },
            doc! { "$set": { "read_at": Utc::now().timestamp_millis() } },
        )
        .await
        .ok();
}

/// Writes a ledger row and moves the cached balance on the profile.
async fn ledger(db: &Database, user_id: &str, points: i64, reason: LoyaltyReason, order_id: Option<String>, note: Option<String>) {
    let now = Utc::now().timestamp_millis();
    let profile = db
        .collection::<CustomerProfile>("customer_profile")
        .find_one_and_update(
            doc! { "user_id": user_id },
            doc! { "$inc": { "loyalty_points": points }, "$set": { "modified_at": now } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await
        .ok()
        .flatten();

    let entry = LoyaltyLedgerEntry {
        uuid: Uuid::now_v7().to_string(),
        user_id: user_id.to_string(),
        points,
        balance_after: profile.map(|p| p.loyalty_points).unwrap_or(points),
        reason,
        order_id,
        note,
        expires_at: None,
        created_at: now,
    };
    db.collection::<LoyaltyLedgerEntry>("loyalty_ledger").insert_one(&entry).await.ok();
}

pub async fn record_redemption(db: &Database, user_id: &str, card: &LoyaltyCard) {
    ledger(db, user_id, -(CARD_SIZE as i64), LoyaltyReason::Redemption, None,
        Some(format!("Card {} redeemed", card.number))).await;
}

/// Called when an order first becomes Delivered.
pub async fn stamp_delivery(db: &Database, user_id: &str, order_id: &str, order_number: &str) {
    let col = db.collection::<LoyaltyCard>(CARDS);

    // One stamp per order, however many times "Delivered" is saved.
    if col.count_documents(doc! { "user_id": user_id, "order_ids": order_id }).await.unwrap_or(0) > 0 {
        return;
    }

    let card = match open_card(db, user_id).await {
        Some(c) => c,
        None => return,
    };

    // The size guard keeps two deliveries landing at once from overfilling
    // a card; the loser of that race starts the next card instead.
    let full_index = format!("order_ids.{}", CARD_SIZE - 1);
    let now = Utc::now().timestamp_millis();
    let pushed = col
        .find_one_and_update(
            doc! { "uuid": &card.uuid, &full_index: { "$exists": false } },
            doc! { "$push": { "order_ids": order_id }, "$set": { "modified_at": now } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await
        .ok()
        .flatten();
    let card = match pushed {
        Some(c) => c,
        None => {
            let next = match open_card(db, user_id).await { Some(c) => c, None => return };
            match col
                .find_one_and_update(
                    doc! { "uuid": &next.uuid },
                    doc! { "$push": { "order_ids": order_id }, "$set": { "modified_at": now } },
                )
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .ok()
                .flatten()
            {
                Some(c) => c,
                None => return,
            }
        }
    };

    ledger(db, user_id, 1, LoyaltyReason::Purchase, Some(order_id.to_string()),
        Some(format!("Stamp for order {}", order_number))).await;

    let stamps = card.stamps();
    let name = customer_name(db, user_id).await;

    if stamps == ALMOST_THERE {
        notify_admins(
            db,
            AdminNotifKind::LoyaltyAlmostThere,
            user_id,
            format!("{} is one stamp from a reward", name),
            if card.reward_coupon_id.is_some() {
                format!("Card {} is at {} of {}. Their reward is already set — check it still fits.", card.number, stamps, CARD_SIZE)
            } else {
                format!("Card {} is at {} of {}. Look over what they've bought and reviewed, and set their reward.", card.number, stamps, CARD_SIZE)
            },
        ).await;
    }

    if stamps >= CARD_SIZE {
        col.update_one(doc! { "uuid": &card.uuid }, doc! { "$set": { "completed_at": now } }).await.ok();

        if card.reward_coupon_id.is_some() {
            send(db, user_id, Notice {
                kind: NotifType::LoyaltyPoints,
                title: "Your card is full — your reward is ready".to_string(),
                body: "Open your stamp card to redeem it.".to_string(),
                image_id: None,
                related_id: Some(card.uuid.clone()),
                link: Some("/account/rewards".to_string()),
                email: None,
            }).await;
        } else {
            send(db, user_id, Notice {
                kind: NotifType::LoyaltyPoints,
                title: "Your card is full!".to_string(),
                body: "Stay tuned — your reward is coming soon.".to_string(),
                image_id: None,
                related_id: Some(card.uuid.clone()),
                link: Some("/account/rewards".to_string()),
                email: None,
            }).await;
            notify_admins(
                db,
                AdminNotifKind::LoyaltyRewardMissing,
                user_id,
                format!("{} filled card {} — no reward set", name, card.number),
                "They've collected all 10 stamps and are waiting on their reward. Set it now.".to_string(),
            ).await;
        }
        return;
    }

    let left = CARD_SIZE - stamps;
    send(db, user_id, Notice {
        kind: NotifType::LoyaltyPoints,
        title: format!("You earned a stamp — {} of {}", stamps, CARD_SIZE),
        body: if left == 1 {
            "One more delivery fills your card.".to_string()
        } else {
            format!("{} more deliveries to your reward.", left)
        },
        image_id: None,
        related_id: Some(card.uuid.clone()),
        link: Some("/account/rewards".to_string()),
        email: None,
    }).await;
}

/// Every card a customer has, oldest first.
pub async fn cards_of(db: &Database, user_id: &str) -> Vec<LoyaltyCard> {
    let mut cards = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<LoyaltyCard>(CARDS)
        .find(doc! { "user_id": user_id })
        .sort(doc! { "number": 1 })
        .await
    {
        while let Some(result) = cursor.next().await {
            match result {
                Ok(c) => cards.push(c),
                Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
            }
        }
    }
    cards
}
