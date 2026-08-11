use serde_json::json;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use super::{resolve_owner, find_cart, build_response};

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let cart = match resolve_owner(&req) {
        Some(owner) => find_cart(&db, &owner).await,
        None => None,
    };

    let body = match cart {
        Some(cart) => build_response(&db, &cart).await,
        // No identity yet, or an identity with no cart on record — an
        // empty cart is still a valid page to render.
        None => json!({
            "uuid": null,
            "items": [],
            "coupon_code": null,
            "coupon_error": null,
            "subtotal": 0,
            "discount": 0,
            "total": 0,
            "item_count": 0,
            "modified_at": null,
        }),
    };

    Ok(HttpResponse::Ok().content_type("application/json").json(body))
}
