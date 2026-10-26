use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::refuse_staff;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cart::{Cart, CartItem};
use super::{resolve_owner, build_response};

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    refuse_staff(&req)?;
    let owner = match resolve_owner(&req) {
        Some(owner) => owner,
        None => return Ok(Response::unauthorized("No cart identity")),
    };

    let db = MongoDB.connect();
    let item_id = path.into_inner();

    let item = db.collection::<CartItem>("cart_item").find_one(doc! { "uuid": &item_id }).await.ok().flatten();
    let item = match item {
        Some(i) => i,
        None => return Ok(Response::not_found("Item not found")),
    };

    let mut cart_filter = owner.filter();
    cart_filter.insert("uuid", item.cart_id.clone());
    let cart = db.collection::<Cart>("cart").find_one(cart_filter).await.ok().flatten();
    let cart = match cart {
        Some(c) => c,
        None => return Ok(Response::forbidden("This item isn't in your cart")),
    };

    db.collection::<CartItem>("cart_item").delete_one(doc! { "uuid": &item_id }).await.ok();
    db.collection::<Cart>("cart")
        .update_one(doc! { "uuid": &cart.uuid }, doc! { "$set": { "modified_at": chrono::Utc::now().timestamp_millis() } })
        .await
        .ok();

    let body = build_response(&db, &cart).await;
    Ok(HttpResponse::Ok().content_type("application/json").json(body))
}
