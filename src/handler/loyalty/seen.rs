use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Loyalty::{LoyaltyCard, CARD_SIZE};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    stamps: u32,
}

/* The card page has played the stamp animation up to `stamps`. */
pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let stamps = body.stamps.min(CARD_SIZE as u32);
    db.collection::<LoyaltyCard>("loyalty_card")
        .update_one(
            doc! { "uuid": path.into_inner(), "user_id": &user.user_id },
            doc! { "$max": { "seen_stamps": stamps } },
        )
        .await
        .ok();

    Ok(Response::ok_message("Seen"))
}
