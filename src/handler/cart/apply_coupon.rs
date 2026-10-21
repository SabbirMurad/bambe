use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cart::Cart;
use super::{resolve_owner, find_cart, compute_subtotal, build_response};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    code: String,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let owner = match resolve_owner(&req) {
        Some(owner) => owner,
        None => return Ok(Response::unauthorized("No cart identity")),
    };

    let db = MongoDB.connect();
    let cart = match find_cart(&db, &owner).await {
        Some(c) => c,
        None => return Ok(Response::not_found("Cart is empty")),
    };

    let (subtotal, product_ids) = compute_subtotal(&db, &cart.uuid).await;
    let user_id = match &owner {
        super::Owner::User(id) => Some(id.as_str()),
        super::Owner::Guest(_) => None,
    };

    if let Err(err) = crate::util::coupon::evaluate(&db, &body.code, user_id, subtotal, &product_ids).await {
        return Ok(Response::bad_request(&err));
    }

    db.collection::<Cart>("cart")
        .update_one(
            doc! { "uuid": &cart.uuid },
            doc! { "$set": { "coupon_code": body.code.to_uppercase(), "modified_at": chrono::Utc::now().timestamp_millis() } },
        )
        .await
        .ok();

    let cart = find_cart(&db, &owner).await.unwrap();
    let response = build_response(&db, &cart).await;
    Ok(HttpResponse::Ok().content_type("application/json").json(response))
}
