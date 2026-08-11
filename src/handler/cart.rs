use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;
use futures::StreamExt;
use serde_json::{json, Value};
use mongodb::bson::doc;
use mongodb::Database;
use actix_web::HttpRequest;
use crate::Middleware::Auth::optional_access;
use crate::model::Cart::{Cart, CartItem};
use crate::model::Product::{Product, ProductVariant};

pub mod get;
pub use get as Get;

pub mod add_item;
pub use add_item as AddItem;

pub mod update_item;
pub use update_item as UpdateItem;

pub mod remove_item;
pub use remove_item as RemoveItem;

pub mod apply_coupon;
pub use apply_coupon as ApplyCoupon;

pub mod remove_coupon;
pub use remove_coupon as RemoveCoupon;

pub mod merge;
pub use merge as Merge;

// Guest header carrying a client-generated id, mirrored back whenever a
// fresh one is minted so the client can start persisting it.
pub const GUEST_HEADER: &str = "X-Guest-Session";

#[derive(Debug, Clone)]
pub enum Owner {
    User(String),
    Guest(String),
}

impl Owner {
    pub fn filter(&self) -> mongodb::bson::Document {
        match self {
            Owner::User(id) => doc! { "user_id": id },
            Owner::Guest(id) => doc! { "session_id": id },
        }
    }
}

/// Signed-in caller wins; otherwise falls back to the guest session header.
/// Returns `None` only when there is truly no identity to key a cart on.
pub fn resolve_owner(req: &HttpRequest) -> Option<Owner> {
    if let Some(user) = optional_access(req) {
        return Some(Owner::User(user.user_id));
    }

    req.headers()
        .get(GUEST_HEADER)
        .and_then(|h| h.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| Owner::Guest(s.to_string()))
}

pub async fn find_cart(db: &Database, owner: &Owner) -> Option<Cart> {
    db.collection::<Cart>("cart").find_one(owner.filter()).await.ok().flatten()
}

pub async fn find_or_create_cart(db: &Database, owner: &Owner) -> Result<Cart, String> {
    if let Some(cart) = find_cart(db, owner).await {
        return Ok(cart);
    }

    let now = Utc::now().timestamp_millis();
    let cart = Cart {
        uuid: Uuid::now_v7().to_string(),
        user_id: match owner {
            Owner::User(id) => Some(id.clone()),
            Owner::Guest(_) => None,
        },
        session_id: match owner {
            Owner::Guest(id) => Some(id.clone()),
            Owner::User(_) => None,
        },
        coupon_code: None,
        created_at: now,
        modified_at: now,
        converted_order_id: None,
    };

    db.collection::<Cart>("cart")
        .insert_one(&cart)
        .await
        .map_err(|e| e.to_string())?;

    Ok(cart)
}

/// Subtotal and the distinct product ids in the cart, over items whose
/// variant still resolves. Used to evaluate a coupon without pulling in all
/// the display fields `build_response` resolves for rendering the page.
pub async fn compute_subtotal(db: &Database, cart_id: &str) -> (i64, Vec<String>) {
    let mut cursor = db
        .collection::<CartItem>("cart_item")
        .find(doc! { "cart_id": cart_id })
        .await
        .expect("cart item lookup");
    let mut items = Vec::new();
    while let Some(Ok(i)) = cursor.next().await {
        items.push(i);
    }

    let variant_ids: Vec<String> = items.iter().map(|i| i.variant_id.clone()).collect();
    let variants: HashMap<String, ProductVariant> = if variant_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<ProductVariant>("product_variant")
            .find(doc! { "uuid": { "$in": &variant_ids } })
            .await
            .expect("variant lookup");
        let mut map = HashMap::new();
        while let Some(Ok(v)) = cursor.next().await {
            map.insert(v.uuid.clone(), v);
        }
        map
    };

    let mut subtotal: i64 = 0;
    let mut product_ids = Vec::new();
    for item in &items {
        if let Some(v) = variants.get(&item.variant_id) {
            subtotal += v.price * item.quantity as i64;
            product_ids.push(item.product_id.clone());
        }
    }

    (subtotal, product_ids)
}

/// Resolves the cart's line items against live catalogue data and totals
/// them up. Used by every cart endpoint that returns a cart, so the shape
/// on the wire is identical whether you just loaded the page or just
/// changed a quantity.
pub async fn build_response(db: &Database, cart: &Cart) -> Value {
    let mut item_cursor = db
        .collection::<CartItem>("cart_item")
        .find(doc! { "cart_id": &cart.uuid })
        .await
        .expect("cart item lookup");
    let mut cart_items = Vec::new();
    while let Some(Ok(i)) = item_cursor.next().await {
        cart_items.push(i);
    }

    let variant_ids: Vec<String> = cart_items.iter().map(|i| i.variant_id.clone()).collect();
    let variants: HashMap<String, ProductVariant> = if variant_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<ProductVariant>("product_variant")
            .find(doc! { "uuid": { "$in": &variant_ids } })
            .await
            .expect("variant lookup");
        let mut map = HashMap::new();
        while let Some(Ok(v)) = cursor.next().await {
            map.insert(v.uuid.clone(), v);
        }
        map
    };

    let product_ids: Vec<String> = cart_items.iter().map(|i| i.product_id.clone()).collect();
    let products: HashMap<String, Product> = if product_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db
            .collection::<Product>("product")
            .find(doc! { "uuid": { "$in": &product_ids } })
            .await
            .expect("product lookup");
        let mut map = HashMap::new();
        while let Some(Ok(p)) = cursor.next().await {
            map.insert(p.uuid.clone(), p);
        }
        map
    };

    let mut subtotal: i64 = 0;
    let mut cart_product_ids: Vec<String> = Vec::new();

    let lines: Vec<Value> = cart_items
        .iter()
        .map(|item| {
            let variant = variants.get(&item.variant_id);
            let product = products.get(&item.product_id);
            let available = variant.is_some() && product.is_some();

            if let (Some(v), Some(_)) = (variant, product) {
                subtotal += v.price * item.quantity as i64;
                cart_product_ids.push(item.product_id.clone());
            }

            // The line's own variant, not the product cover — a cart showing
            // the black one when you added the tan one is just wrong.
            let line_image_id = variant.map(|v| v.image_id.clone());

            json!({
                "uuid": item.uuid,
                "product_id": item.product_id,
                "variant_id": item.variant_id,
                "title": product.map(|p| p.title.clone()),
                "size": variant.and_then(|v| v.size.clone()),
                "color": variant.and_then(|v| v.color.clone()),
                "image_id": line_image_id,
                "quantity": item.quantity,
                "unit_price": variant.map(|v| v.price),
                "price_when_added": item.price_when_added,
                "price_changed": variant.map(|v| v.price != item.price_when_added).unwrap_or(false),
                "line_total": variant.map(|v| v.price * item.quantity as i64),
                "in_stock": variant.map(|v| v.stock >= item.quantity as i32).unwrap_or(false),
                "available": available,
                "added_at": item.added_at,
            })
        })
        .collect();

    let mut discount: i64 = 0;
    let mut coupon_error: Option<String> = None;
    if let Some(code) = &cart.coupon_code {
        match crate::utils::coupon::evaluate(db, code, cart.user_id.as_deref(), subtotal, &cart_product_ids).await {
            Ok(applied) => discount = applied.discount,
            Err(err) => coupon_error = Some(err),
        }
    }

    json!({
        "uuid": cart.uuid,
        "items": lines,
        "coupon_code": cart.coupon_code,
        "coupon_error": coupon_error,
        "subtotal": subtotal,
        "discount": discount,
        "total": subtotal - discount,
        "item_count": cart_items.iter().map(|i| i.quantity).sum::<u32>(),
        "modified_at": cart.modified_at,
    })
}
