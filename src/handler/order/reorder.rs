use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Order::{Order, OrderItem};
use crate::model::Product::ProductVariant;
use crate::model::Cart::CartItem;
use crate::Handler::Cart::{find_or_create_cart, Owner};

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let order_id = path.into_inner();

    let order = db
        .collection::<Order>("order")
        .find_one(doc! { "uuid": &order_id, "user_id": &user.user_id })
        .await
        .ok()
        .flatten();
    if order.is_none() {
        return Ok(Response::not_found("Order not found"));
    }

    let mut item_cursor = db
        .collection::<OrderItem>("order_item")
        .find(doc! { "order_id": &order_id })
        .await
        .expect("order item lookup");
    let mut items = Vec::new();
    while let Some(Ok(i)) = item_cursor.next().await {
        items.push(i);
    }

    let owner = Owner::User(user.user_id.clone());
    let cart = match find_or_create_cart(&db, &owner).await {
        Ok(c) => c,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error));
        }
    };

    let item_col = db.collection::<CartItem>("cart_item");
    let mut added = Vec::new();
    let mut unavailable = Vec::new();
    let now = Utc::now().timestamp_millis();

    for line in &items {
        let variant = db
            .collection::<ProductVariant>("product_variant")
            .find_one(doc! { "uuid": &line.variant_id })
            .await
            .ok()
            .flatten();

        let variant = match variant {
            Some(v) if v.stock > 0 => v,
            _ => {
                unavailable.push(json!({ "product_id": line.product_id, "title": line.title }));
                continue;
            }
        };

        let existing = item_col
            .find_one(doc! { "cart_id": &cart.uuid, "variant_id": &line.variant_id })
            .await
            .ok()
            .flatten();

        if let Some(existing) = existing {
            item_col
                .update_one(doc! { "uuid": &existing.uuid }, doc! { "$inc": { "quantity": line.quantity as i32 } })
                .await
                .ok();
        } else {
            let item = CartItem {
                uuid: Uuid::now_v7().to_string(),
                cart_id: cart.uuid.clone(),
                product_id: variant.product_id.clone(),
                variant_id: variant.uuid.clone(),
                quantity: line.quantity,
                added_at: now,
                price_when_added: variant.price,
            };
            item_col.insert_one(&item).await.ok();
        }

        added.push(json!({ "product_id": line.product_id, "title": line.title }));
    }

    db.collection::<crate::model::Cart::Cart>("cart")
        .update_one(doc! { "uuid": &cart.uuid }, doc! { "$set": { "modified_at": now } })
        .await
        .ok();

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "added": added,
        "unavailable": unavailable,
    })))
}
