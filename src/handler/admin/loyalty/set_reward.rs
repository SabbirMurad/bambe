use chrono::Utc;
use uuid::Uuid;
use rand::Rng;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Account::AccountCore;
use crate::model::Cents;
use crate::model::Coupon::{Coupon, DiscountType};
use crate::model::Loyalty::LoyaltyCard;
use crate::model::Notification::NotifType;
use crate::util::notify::{send, Notice};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    discount_type: DiscountType,
    // Basis points for Percentage (2000 = 20%), cents for Fixed.
    #[serde(default)]
    value: i64,
    max_discount: Option<Cents>,
    min_spend: Option<Cents>,
    // Days the code stays usable after the customer redeems it.
    valid_days: Option<u32>,
    #[serde(default)]
    description: String,
}

fn new_code() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    let tail: String = (0..6).map(|_| CHARS[rng.random_range(0..CHARS.len())] as char).collect();
    format!("BMB-{}", tail)
}

/*
  Sets the reward on the customer's next card to pay out — a full card
  waiting on one first, otherwise the card being filled (created if the
  customer has none yet, so a reward can be lined up before any stamps).

  The reward is a personal coupon: only this account can use it, once, and
  it stays switched off until the card is redeemed. Setting it again edits
  the same coupon rather than minting another.
*/
pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Customers).await?;
    let db = MongoDB.connect();
    let user_id = path.into_inner();

    if db.collection::<AccountCore>("account_core").find_one(doc! { "uuid": &user_id }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Customer not found"));
    }

    match body.discount_type {
        DiscountType::Percentage if !(1..=10000).contains(&body.value) => {
            return Ok(Response::bad_request("A percentage must be between 0.01% and 100%"));
        }
        DiscountType::Fixed if body.value <= 0 => {
            return Ok(Response::bad_request("Enter an amount off"));
        }
        _ => {}
    }
    if body.valid_days == Some(0) {
        return Ok(Response::bad_request("Valid for at least one day, or leave it blank"));
    }

    let card = match crate::util::loyalty::reward_card(&db, &user_id).await {
        Some(c) => c,
        None => return Ok(Response::internal_server_error("Couldn't open a loyalty card")),
    };

    let now = Utc::now().timestamp_millis();
    let coupons = db.collection::<Coupon>("coupon");
    let customer = db.collection::<AccountCore>("account_core").find_one(doc! { "uuid": &user_id }).await.ok().flatten();
    // What the coupons list shows staff; the customer sees `message`.
    let description = format!(
        "Loyalty reward for {} — card {}",
        customer.map(|c| c.username).unwrap_or_else(|| "a customer".to_string()),
        card.number,
    );
    let message = Some(body.description.trim().to_string()).filter(|m| !m.is_empty());
    let value = if body.discount_type == DiscountType::FreeShipping { 0 } else { body.value };

    let existing = match &card.reward_coupon_id {
        Some(id) => coupons.find_one(doc! { "uuid": id }).await.ok().flatten(),
        None => None,
    };

    let coupon_id = match existing {
        Some(c) => {
            coupons.update_one(doc! { "uuid": &c.uuid }, doc! { "$set": {
                "description": &description,
                "discount_type": mongodb::bson::to_bson(&body.discount_type).unwrap(),
                "value": value,
                "max_discount": body.max_discount,
                "min_spend": body.min_spend,
                "modified_at": now,
            } }).await.ok();
            c.uuid
        }
        None => {
            let mut code = new_code();
            while coupons.count_documents(doc! { "code": &code }).await.unwrap_or(0) > 0 {
                code = new_code();
            }
            let coupon = Coupon {
                uuid: Uuid::now_v7().to_string(),
                code,
                description: description.clone(),
                discount_type: body.discount_type.clone(),
                value,
                max_discount: body.max_discount,
                min_spend: body.min_spend,
                total_limit: Some(1),
                per_user_limit: Some(1),
                used_count: 0,
                starts_at: now,
                ends_at: None,
                // Goes live when the customer redeems the card.
                enabled: false,
                product_ids: Vec::new(),
                category_ids: Vec::new(),
                first_order_only: false,
                user_id: Some(user_id.clone()),
                created_at: now,
                created_by: admin.user_id.clone(),
                modified_at: now,
            };
            if let Err(error) = coupons.insert_one(&coupon).await {
                log::error!("{:?}", error);
                return Ok(Response::internal_server_error(&error.to_string()));
            }
            coupon.uuid
        }
    };

    db.collection::<LoyaltyCard>("loyalty_card")
        .update_one(doc! { "uuid": &card.uuid }, doc! { "$set": {
            "reward_coupon_id": &coupon_id,
            "reward_valid_days": body.valid_days,
            "reward_message": message.clone(),
            "modified_at": now,
        } })
        .await
        .ok();

    // A full card was waiting on this: tell the customer, and clear the
    // console's alerts about them.
    if card.is_full() {
        crate::util::loyalty::resolve_admin_alerts(&db, &user_id).await;
        if card.reward_coupon_id.is_none() {
            send(&db, &user_id, Notice {
                kind: NotifType::LoyaltyPoints,
                title: "Your reward is ready".to_string(),
                body: "Thanks for waiting — open your stamp card to redeem it.".to_string(),
                image_id: None,
                related_id: Some(card.uuid.clone()),
                link: Some("/account/rewards".to_string()),
                email: None,
            }).await;
        }
    } else if card.stamps() >= crate::model::Loyalty::ALMOST_THERE {
        crate::util::loyalty::resolve_admin_alerts(&db, &user_id).await;
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "loyalty.reward_set", "loyalty_card", &card.uuid, Some(description),
    ).await;

    let card = db.collection::<LoyaltyCard>("loyalty_card").find_one(doc! { "uuid": &card.uuid }).await.ok().flatten().unwrap_or(card);
    Ok(HttpResponse::Ok().content_type("application/json").json(crate::Handler::Loyalty::card_view(&db, &card, true).await))
}
