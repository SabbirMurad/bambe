use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use serde_json::{json, Value};
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Cart::CartItem;
use crate::model::Product::ProductVariant;
use super::{resolve_owner, find_or_create_cart, build_response, Owner};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    variant_id: String,
    #[serde(default = "default_quantity")]
    quantity: u32,
}
fn default_quantity() -> u32 { 1 }

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    if body.quantity == 0 {
        return Ok(Response::bad_request("Quantity must be at least 1"));
    }

    let db = MongoDB.connect();

    // A first-time guest gets a session id minted here and handed back in
    // the response so the client can start sending it on every later call.
    let (owner, minted_session_id) = match resolve_owner(&req) {
        Some(owner) => (owner, None),
        None => {
            let id = Uuid::now_v7().to_string();
            (Owner::Guest(id.clone()), Some(id))
        }
    };

    let variant = db
        .collection::<ProductVariant>("product_variant")
        .find_one(doc! { "uuid": &body.variant_id })
        .await
        .ok()
        .flatten();

    let variant = match variant {
        Some(v) => v,
        None => return Ok(Response::not_found("Variant not found")),
    };

    let cart = match find_or_create_cart(&db, &owner).await {
        Ok(cart) => cart,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error));
        }
    };

    let item_col = db.collection::<CartItem>("cart_item");
    let existing = item_col
        .find_one(doc! { "cart_id": &cart.uuid, "variant_id": &body.variant_id })
        .await
        .ok()
        .flatten();

    let now = Utc::now().timestamp_millis();

    if let Some(existing) = existing {
        let result = item_col
            .update_one(
                doc! { "uuid": &existing.uuid },
                doc! { "$inc": { "quantity": body.quantity as i32 } },
            )
            .await;
        if let Err(error) = result {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    } else {
        let item = CartItem {
            uuid: Uuid::now_v7().to_string(),
            cart_id: cart.uuid.clone(),
            product_id: variant.product_id.clone(),
            variant_id: variant.uuid.clone(),
            quantity: body.quantity,
            added_at: now,
            price_when_added: variant.price,
        };
        if let Err(error) = item_col.insert_one(&item).await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    db.collection::<crate::model::Cart::Cart>("cart")
        .update_one(doc! { "uuid": &cart.uuid }, doc! { "$set": { "modified_at": now } })
        .await
        .ok();

    let mut body = build_response(&db, &cart).await;
    if let (Some(id), Value::Object(map)) = (minted_session_id, &mut body) {
        map.insert("session_id".to_string(), json!(id));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(body))
}
