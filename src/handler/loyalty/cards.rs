use serde_json::json;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Loyalty::CARD_SIZE;

/*
  The caller's stamp cards: the one being filled, any full ones waiting to
  be redeemed, and the rewards already redeemed (with their codes). A
  customer with no deliveries yet gets an empty first card to look at.
*/
pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let cards = crate::util::loyalty::cards_of(&db, &user.user_id).await;

    let mut current = None;
    let mut full = Vec::new();
    let mut redeemed = Vec::new();
    for card in &cards {
        let view = super::card_view(&db, card, false).await;
        if card.redeemed_at.is_some() {
            redeemed.push(view);
        } else if card.is_full() {
            full.push(view);
        } else {
            current = Some(view);
        }
    }
    redeemed.reverse();

    let current = current.unwrap_or_else(|| json!({
        "uuid": null,
        "number": cards.last().map(|c| c.number + 1).unwrap_or(1),
        "size": CARD_SIZE,
        "stamps": 0,
        "seen_stamps": 0,
        "status": "collecting",
        "reward_ready": false,
        "reward": null,
    }));

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "size": CARD_SIZE,
        "current": current,
        "full": full,
        "redeemed": redeemed,
    })))
}
