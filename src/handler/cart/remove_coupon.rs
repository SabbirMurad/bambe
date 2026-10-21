use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cart::Cart;
use super::{resolve_owner, find_cart, build_response};

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let owner = match resolve_owner(&req) {
        Some(owner) => owner,
        None => return Ok(Response::unauthorized("No cart identity")),
    };

    let db = MongoDB.connect();
    let cart = match find_cart(&db, &owner).await {
        Some(c) => c,
        None => return Ok(Response::not_found("Cart is empty")),
    };

    db.collection::<Cart>("cart")
        .update_one(
            doc! { "uuid": &cart.uuid },
            doc! { "$set": { "coupon_code": mongodb::bson::Bson::Null, "modified_at": chrono::Utc::now().timestamp_millis() } },
        )
        .await
        .ok();

    let cart = find_cart(&db, &owner).await.unwrap();
    let response = build_response(&db, &cart).await;
    Ok(HttpResponse::Ok().content_type("application/json").json(response))
}
