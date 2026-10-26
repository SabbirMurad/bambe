use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Account::{AccountCore, AccountProfile};
use crate::model::Customer::CustomerProfile;
use crate::model::Order::{FulfilmentStatus, Order, OrderItem};
use crate::model::Product::Product;
use crate::model::Review::ProductReview;
use std::collections::HashMap;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
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

    /*
      What staff read before choosing a loyalty reward: what this customer
      buys and how they review. Delivered orders only for the purchases —
      a cancelled cart says nothing about taste.
    */
    let mut delivered = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<Order>("order")
        .find(doc! { "user_id": &user_id, "fulfilment_status": "Delivered" })
        .await
    {
        while let Some(Ok(o)) = cursor.next().await {
            if o.fulfilment_status == FulfilmentStatus::Delivered { delivered.push(o.uuid); }
        }
    }
    let mut bought: HashMap<String, (String, u32, i64)> = HashMap::new();
    if !delivered.is_empty() {
        if let Ok(mut cursor) = db.collection::<OrderItem>("order_item").find(doc! { "order_id": { "$in": &delivered } }).await {
            while let Some(Ok(line)) = cursor.next().await {
                let entry = bought.entry(line.product_id.clone()).or_insert((line.title.clone(), 0, 0));
                entry.1 += line.quantity;
                entry.2 += line.line_total;
            }
        }
    }
    let mut purchases: Vec<_> = bought.into_iter().collect();
    purchases.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(b.1 .2.cmp(&a.1 .2)));
    let purchases: Vec<_> = purchases
        .iter()
        .take(12)
        .map(|(id, (title, qty, spent))| json!({ "product_id": id, "title": title, "quantity": qty, "spent": spent }))
        .collect();

    let mut reviews = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<ProductReview>("product_review")
        .find(doc! { "user_id": &user_id, "deleted_at": null })
        .sort(doc! { "created_at": -1 })
        .await
    {
        while let Some(Ok(r)) = cursor.next().await {
            reviews.push(r);
        }
    }
    // A whole-order review is one review however many products it is on.
    let mut seen_groups = std::collections::HashSet::new();
    let reviews: Vec<&ProductReview> = reviews
        .iter()
        .filter(|r| r.group_id.as_ref().map(|g| seen_groups.insert(g.clone())).unwrap_or(true))
        .collect();
    let review_avg = if reviews.is_empty() { 0.0 } else {
        reviews.iter().map(|r| r.rating as f64).sum::<f64>() / reviews.len() as f64
    };
    let review_product_ids: Vec<String> = reviews.iter().take(5).map(|r| r.product_id.clone()).collect();
    let mut titles: HashMap<String, String> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Product>("product").find(doc! { "uuid": { "$in": &review_product_ids } }).await {
        while let Some(Ok(p)) = cursor.next().await {
            titles.insert(p.uuid, p.title);
        }
    }
    let recent_reviews: Vec<_> = reviews
        .iter()
        .take(5)
        .map(|r| json!({
            "rating": r.rating,
            "title": r.title,
            "body": r.body,
            "product": titles.get(&r.product_id),
            "scope": r.scope,
            "status": r.status.to_string(),
            "created_at": r.created_at,
        }))
        .collect();

    let mut cards = Vec::new();
    for card in crate::util::loyalty::cards_of(&db, &user_id).await.iter().rev().take(4) {
        cards.push(crate::Handler::Loyalty::card_view(&db, card, true).await);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "purchases": purchases,
        "reviews": { "count": reviews.len(), "average": (review_avg * 10.0).round() / 10.0, "recent": recent_reviews },
        // Newest first; staff see the reward details even before redemption.
        "loyalty_cards": cards,
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
