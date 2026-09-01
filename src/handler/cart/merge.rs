use serde::Deserialize;
use futures::StreamExt;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Cart::{Cart, CartItem};
use super::{find_or_create_cart, build_response, Owner, GUEST_HEADER};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    session_id: Option<String>,
}

/*
  Folds the cart a shopper built before signing in into their account cart.
  Called right after sign-in/sign-up with the guest session id the client
  was carrying. Matching variants add their quantities together rather than
  duplicating the line.
*/
pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let session_id = body
        .session_id
        .clone()
        .or_else(|| req.headers().get(GUEST_HEADER).and_then(|h| h.to_str().ok()).map(|s| s.to_string()));

    let session_id = match session_id {
        Some(id) if !id.is_empty() => id,
        _ => return Ok(Response::bad_request("No guest session to merge")),
    };

    let guest_cart = db
        .collection::<Cart>("cart")
        .find_one(doc! { "session_id": &session_id })
        .await
        .ok()
        .flatten();

    let user_owner = Owner::User(user.user_id.clone());
    let user_cart = match find_or_create_cart(&db, &user_owner).await {
        Ok(c) => c,
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error));
        }
    };

    if let Some(guest_cart) = guest_cart {
        let item_col = db.collection::<CartItem>("cart_item");
        let mut cursor = item_col
            .find(doc! { "cart_id": &guest_cart.uuid })
            .await
            .expect("guest cart item lookup");
        let mut guest_items = Vec::new();
        while let Some(result) = cursor.next().await {
            let i = match result {
                Ok(i) => i,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            guest_items.push(i);
        }

        for item in guest_items {
            let existing = item_col
                .find_one(doc! { "cart_id": &user_cart.uuid, "variant_id": &item.variant_id })
                .await
                .ok()
                .flatten();

            if let Some(existing) = existing {
                item_col
                    .update_one(
                        doc! { "uuid": &existing.uuid },
                        doc! { "$inc": { "quantity": item.quantity as i32 } },
                    )
                    .await
                    .ok();
                item_col.delete_one(doc! { "uuid": &item.uuid }).await.ok();
            } else {
                item_col
                    .update_one(doc! { "uuid": &item.uuid }, doc! { "$set": { "cart_id": &user_cart.uuid } })
                    .await
                    .ok();
            }
        }

        if user_cart.coupon_code.is_none() {
            if let Some(code) = &guest_cart.coupon_code {
                db.collection::<Cart>("cart")
                    .update_one(doc! { "uuid": &user_cart.uuid }, doc! { "$set": { "coupon_code": code } })
                    .await
                    .ok();
            }
        }

        db.collection::<Cart>("cart").delete_one(doc! { "uuid": &guest_cart.uuid }).await.ok();
    }

    let merged = db.collection::<Cart>("cart").find_one(doc! { "uuid": &user_cart.uuid }).await.ok().flatten().unwrap_or(user_cart);
    let response = build_response(&db, &merged).await;
    Ok(HttpResponse::Ok().content_type("application/json").json(response))
}
