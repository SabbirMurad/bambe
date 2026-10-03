//! The sender behind the notification bell and the transactional emails.
//!
//! `notification` had a full read side — list, mark-read, mark-all-read,
//! preferences — and no writer at all, so the bell was permanently empty. This
//! is that writer, and it is the single place that decides whether a given user
//! wants a given kind of message.
//!
//! Preference semantics come from `model::Customer::NotificationPreferences`:
//! a topic goes out only where both its own flag and the channel flag are on.
//! The in-app row is not a channel — it *is* the list the bell reads, so
//! `push_enabled` (FCM) and `email_enabled` don't gate it; only the topic does.
//!
//! Everything here is best-effort. A notification that fails to write must
//! never fail the order it is describing, so callers don't get a Result back.

use chrono::Utc;
use uuid::Uuid;
use mongodb::bson::doc;
use crate::model::Customer::NotificationPreferences;
use crate::model::Notification::{AppNotification, NotifType};
use crate::Model::Account::AccountCore;

/// One message to one user, on whichever channels they allow.
pub struct Notice {
    pub kind: NotifType,
    pub title: String,
    pub body: String,
    pub image_id: Option<String>,
    pub related_id: Option<String>,
    pub link: Option<String>,
    /// Rendered HTML for the email copy. `None` means in-app only.
    pub email: Option<Email>,
}

pub struct Email {
    pub subject: String,
    pub html: String,
}

/// Which preference flag governs a type. `None` means the type is not
/// suppressible — an account-security message goes out regardless.
fn topic_allowed(kind: &NotifType, prefs: &Prefs) -> bool {
    match kind {
        NotifType::OrderPlaced
        | NotifType::OrderConfirmed
        | NotifType::OrderCancelled
        | NotifType::RefundIssued => prefs.order_updates,

        NotifType::OrderShipped | NotifType::OrderDelivered => prefs.delivery_updates,

        NotifType::SupportReply => prefs.support_replies,
        NotifType::PriceDrop => prefs.price_drops,
        NotifType::BackInStock => prefs.back_in_stock,
        NotifType::ReviewReminder => prefs.review_reminders,
        NotifType::Promotion | NotifType::LoyaltyPoints => prefs.promotions,

        // Security and system messages are not opt-out.
        NotifType::Security | NotifType::System => true,
    }
}

struct Prefs {
    order_updates: bool,
    delivery_updates: bool,
    support_replies: bool,
    price_drops: bool,
    back_in_stock: bool,
    review_reminders: bool,
    promotions: bool,
    email_enabled: bool,
}

impl Default for Prefs {
    /// Must match the `None` branch of `notification/get_preferences.rs`, so
    /// the screen and the sender agree on what "never set" means.
    fn default() -> Self {
        Prefs {
            order_updates: true,
            delivery_updates: true,
            support_replies: true,
            price_drops: true,
            back_in_stock: true,
            review_reminders: true,
            promotions: false,
            email_enabled: true,
        }
    }
}

async fn load_prefs(db: &mongodb::Database, user_id: &str) -> Prefs {
    match db
        .collection::<NotificationPreferences>("notification_preferences")
        .find_one(doc! { "user_id": user_id })
        .await
    {
        Ok(Some(p)) => Prefs {
            order_updates: p.order_updates,
            delivery_updates: p.delivery_updates,
            support_replies: p.support_replies,
            price_drops: p.price_drops,
            back_in_stock: p.back_in_stock,
            review_reminders: p.review_reminders,
            promotions: p.promotions,
            email_enabled: p.email_enabled,
        },
        Ok(None) => Prefs::default(),
        Err(error) => {
            // Failing open on a read error would spam someone who opted out;
            // failing closed would silently drop an order confirmation. The
            // defaults are the honest middle, and the error is logged.
            log::error!("notification preferences for {}: {:?}", user_id, error);
            Prefs::default()
        }
    }
}

/// Write the in-app row and, when the notice carries email copy and the user
/// takes email, send that too.
pub async fn send(db: &mongodb::Database, user_id: &str, notice: Notice) {
    let prefs = load_prefs(db, user_id).await;

    if !topic_allowed(&notice.kind, &prefs) {
        return;
    }

    let row = AppNotification {
        uuid: Uuid::now_v7().to_string(),
        recipient_id: user_id.to_string(),
        r#type: notice.kind,
        title: notice.title.clone(),
        body: notice.body.clone(),
        image_id: notice.image_id,
        related_id: notice.related_id,
        link: notice.link,
        is_read: false,
        created_at: Utc::now().timestamp_millis(),
    };

    if let Err(error) = db
        .collection::<AppNotification>("notification")
        .insert_one(&row)
        .await
    {
        log::error!("could not write notification for {}: {:?}", user_id, error);
    }

    let email = match notice.email {
        Some(email) if prefs.email_enabled => email,
        _ => return,
    };

    let address = db
        .collection::<AccountCore>("account_core")
        .find_one(doc! { "uuid": user_id })
        .await
        .ok()
        .flatten()
        .map(|a| a.email_address);

    let address = match address {
        Some(address) if !address.is_empty() => address,
        _ => {
            log::error!("no email address for {}, notification sent in-app only", user_id);
            return;
        }
    };

    if let Err(error) = crate::Integrations::Resend::send_email(&address, &email.subject, &email.html).await {
        // Already logged inside send_email; this names what was being sent.
        log::error!("order notification email to {} failed: {:?}", address, error);
    }
}
