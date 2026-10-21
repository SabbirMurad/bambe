use chrono::Utc;
use mongodb::bson::{doc, Bson};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Coupon::Coupon;
use crate::model::Loyalty::LoyaltyCard;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/*
  Trades a full card for its reward. The personal coupon was created
  switched off; this is where it goes live and its validity window starts.
*/
pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<LoyaltyCard>("loyalty_card");
    let id = path.into_inner();

    let card = match col.find_one(doc! { "uuid": &id, "user_id": &user.user_id }).await.ok().flatten() {
        Some(c) => c,
        None => return Ok(Response::not_found("Card not found")),
    };
    if !card.is_full() {
        return Ok(Response::bad_request("Fill the card first"));
    }
    let coupon_id = match &card.reward_coupon_id {
        Some(id) => id.clone(),
        None => return Ok(Response::bad_request("Your reward is on its way — stay tuned")),
    };

    let now = Utc::now().timestamp_millis();
    // Claimed atomically, so a double tap can't redeem twice.
    let claimed = col
        .update_one(
            doc! { "uuid": &id, "redeemed_at": null },
            doc! { "$set": { "redeemed_at": now, "modified_at": now } },
        )
        .await
        .map(|r| r.modified_count == 1)
        .unwrap_or(false);
    if !claimed {
        return Ok(Response::bad_request("This card has already been redeemed"));
    }

    let ends_at = match card.reward_valid_days {
        Some(days) => Bson::Int64(now + days as i64 * DAY_MS),
        None => Bson::Null,
    };
    db.collection::<Coupon>("coupon")
        .update_one(
            doc! { "uuid": &coupon_id },
            doc! { "$set": { "enabled": true, "starts_at": now, "ends_at": ends_at, "modified_at": now } },
        )
        .await
        .ok();

    crate::utils::loyalty::record_redemption(&db, &user.user_id, &card).await;

    let card = col.find_one(doc! { "uuid": &id }).await.ok().flatten().unwrap_or(card);
    let view = super::card_view(&db, &card, false).await;
    Ok(HttpResponse::Ok().content_type("application/json").json(view))
}
