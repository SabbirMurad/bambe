use chrono::Utc;
use mongodb::bson::{doc, Bson};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Coupon::Coupon;
use crate::model::Loyalty::LoyaltyCard;

/* Takes the reward back off a card that hasn't been redeemed yet. */
pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let user_id = path.into_inner();

    let col = db.collection::<LoyaltyCard>("loyalty_card");
    let card = col
        .find_one(doc! { "user_id": &user_id, "redeemed_at": null, "reward_coupon_id": { "$ne": null } })
        .sort(doc! { "number": 1 })
        .await
        .ok()
        .flatten();
    let card = match card {
        Some(c) => c,
        None => return Ok(Response::not_found("No reward to remove")),
    };

    if let Some(id) = &card.reward_coupon_id {
        db.collection::<Coupon>("coupon").delete_one(doc! { "uuid": id, "used_count": 0 }).await.ok();
    }
    col.update_one(doc! { "uuid": &card.uuid }, doc! { "$set": {
        "reward_coupon_id": Bson::Null,
        "reward_valid_days": Bson::Null,
        "reward_message": Bson::Null,
        "modified_at": Utc::now().timestamp_millis(),
    } }).await.ok();

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(
        &db, &admin.user_id, &name, "loyalty.reward_removed", "loyalty_card", &card.uuid, None,
    ).await;

    Ok(Response::ok_message("Reward removed"))
}
