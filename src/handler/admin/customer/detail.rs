use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::{AccountRole, AccountCore, AccountProfile};
use crate::model::Customer::CustomerProfile;
use crate::model::Order::Order;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let user_id = path.into_inner();

    let core = db.collection::<AccountCore>("account_core").find_one(doc! { "uuid": &user_id, "role": "User" }).await.ok().flatten();
    let core = match core {
        Some(c) => c,
        None => return Ok(Response::not_found("Customer not found")),
    };

    let profile = db.collection::<AccountProfile>("account_profile").find_one(doc! { "uuid": &user_id }).await.ok().flatten();
    let customer = db.collection::<CustomerProfile>("customer_profile").find_one(doc! { "user_id": &user_id }).await.ok().flatten();

    let mut order_cursor = db
        .collection::<Order>("order")
        .find(doc! { "user_id": &user_id })
        .sort(doc! { "placed_at": -1 })
        .limit(10)
        .await
        .expect("order lookup");
    let mut orders = Vec::new();
    while let Some(result) = order_cursor.next().await {
        let o = match result {
            Ok(o) => o,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        orders.push(json!({
            "uuid": o.uuid,
            "number": o.number,
            "fulfilment_status": o.fulfilment_status,
            "payment_status": o.payment_status,
            "grand_total": o.totals.grand_total,
            "placed_at": o.placed_at,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "user_id": core.uuid,
        "email": core.email_address,
        "username": core.username,
        "created_at": core.created_at,
        "first_name": profile.as_ref().map(|p| p.first_name.clone()),
        "last_name": profile.as_ref().map(|p| p.last_name.clone()),
        "phone_number": profile.as_ref().and_then(|p| p.phone_number.clone()),
        "segment": customer.as_ref().map(|c| c.segment.clone()).unwrap_or_default(),
        "order_count": customer.as_ref().map(|c| c.order_count).unwrap_or(0),
        "lifetime_spend": customer.as_ref().map(|c| c.lifetime_spend).unwrap_or(0),
        "loyalty_points": customer.as_ref().map(|c| c.loyalty_points).unwrap_or(0),
        "location": customer.as_ref().and_then(|c| c.location.clone()),
        "note": customer.as_ref().and_then(|c| c.note.clone()),
        "tags": customer.as_ref().map(|c| c.tags.clone()).unwrap_or_default(),
        "accepts_marketing": customer.as_ref().map(|c| c.accepts_marketing).unwrap_or(false),
        "recent_orders": orders,
    })))
}
