use serde_json::json;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Order::Order;
use crate::model::Cart::WishlistItem;
use crate::model::Review::ProductReview;
use crate::model::Customer::CustomerProfile;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let order_count = db.collection::<Order>("order").count_documents(doc! { "user_id": &user.user_id }).await.unwrap_or(0);
    let wishlist_count = db.collection::<WishlistItem>("wishlist_item").count_documents(doc! { "user_id": &user.user_id }).await.unwrap_or(0);
    let review_count = db
        .collection::<ProductReview>("product_review")
        .count_documents(doc! { "user_id": &user.user_id, "deleted_at": null })
        .await
        .unwrap_or(0);

    let customer = db.collection::<CustomerProfile>("customer_profile").find_one(doc! { "user_id": &user.user_id }).await.ok().flatten();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "order_count": order_count,
        "wishlist_count": wishlist_count,
        "review_count": review_count,
        "loyalty_points": customer.as_ref().map(|c| c.loyalty_points).unwrap_or(0),
        "segment": customer.map(|c| c.segment).unwrap_or_default(),
    })))
}
