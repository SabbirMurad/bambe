use mongodb::bson::doc;
use mongodb::Database;
use futures::StreamExt;
use crate::model::Cents;
use crate::model::Coupon::{Coupon, CouponRedemption, DiscountType};
use crate::model::Order::Order;
use crate::model::Product::Product;

pub struct AppliedDiscount {
    pub coupon: Coupon,
    pub discount: Cents,
    pub free_shipping: bool,
}

/*
  Shared between "apply to cart" and "place order" — the order placement
  re-runs this against the cart's contents at the moment of purchase rather
  than trusting whatever the cart said, because a code can expire or hit its
  limit in the minutes between the two.
*/
pub async fn evaluate(
    db: &Database,
    code: &str,
    user_id: Option<&str>,
    subtotal: Cents,
    cart_product_ids: &[String],
) -> Result<AppliedDiscount, String> {
    let now = chrono::Utc::now().timestamp_millis();

    let coupon = db
        .collection::<Coupon>("coupon")
        .find_one(doc! { "code": code.to_uppercase() })
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Coupon not found".to_string())?;

    if let Some(owner) = &coupon.user_id {
        if user_id != Some(owner.as_str()) {
            return Err("This code belongs to another account".to_string());
        }
    }
    if !coupon.enabled {
        return Err("This coupon is disabled".to_string());
    }
    if coupon.starts_at > now {
        return Err("This coupon is not active yet".to_string());
    }
    if coupon.ends_at.map(|end| end < now).unwrap_or(false) {
        return Err("This coupon has expired".to_string());
    }
    if let Some(limit) = coupon.total_limit {
        if coupon.used_count >= limit {
            return Err("This coupon has been fully redeemed".to_string());
        }
    }
    if let Some(min_spend) = coupon.min_spend {
        if subtotal < min_spend {
            // Cents are the wire format, not something to show a shopper —
            // this read "a minimum spend of 2000".
            return Err(format!(
                "This coupon needs a minimum spend of ${}.{:02}",
                min_spend / 100,
                min_spend % 100,
            ));
        }
    }

    if let Some(user_id) = user_id {
        if let Some(limit) = coupon.per_user_limit {
            let used = db
                .collection::<CouponRedemption>("coupon_redemption")
                .count_documents(doc! { "coupon_id": &coupon.uuid, "user_id": user_id })
                .await
                .map_err(|e| e.to_string())?;
            if used >= limit as u64 {
                return Err("You've already used this coupon the maximum number of times".to_string());
            }
        }

        if coupon.first_order_only {
            /*
              "First order" means they have ordered before, not that the money
              cleared. Testing `payment_status: "Paid"` missed cash-on-delivery
              entirely — COD stays Pending until the courier collects, so a COD
              customer could reuse a first-order coupon forever. Any order that
              wasn't cancelled counts.
            */
            let has_previous_order = db
                .collection::<Order>("order")
                .count_documents(doc! {
                    "user_id": user_id,
                    "fulfilment_status": { "$ne": "Cancelled" },
                })
                .await
                .map_err(|e| e.to_string())?
                > 0;
            if has_previous_order {
                return Err("This coupon is only valid on a first order".to_string());
            }
        }
    }

    if !coupon.product_ids.is_empty() || !coupon.category_ids.is_empty() {
        let matches_product = !coupon.product_ids.is_empty()
            && cart_product_ids.iter().any(|id| coupon.product_ids.contains(id));

        let matches_category = if coupon.category_ids.is_empty() {
            false
        } else {
            let mut cursor = db
                .collection::<Product>("product")
                .find(doc! { "uuid": { "$in": cart_product_ids } })
                .await
                .map_err(|e| e.to_string())?;
            let mut hit = false;
            while let Some(result) = cursor.next().await {
                let p = match result {
                    Ok(p) => p,
                    Err(error) => {
                        log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                        continue;
                    }
                };
                if coupon.category_ids.contains(&p.category_id) {
                    hit = true;
                    break;
                }
            }
            hit
        };

        if !matches_product && !matches_category {
            return Err("This coupon doesn't apply to the items in your cart".to_string());
        }
    }

    let discount = match coupon.discount_type {
        DiscountType::Percentage => {
            let raw = subtotal * coupon.value / 10_000;
            coupon.max_discount.map(|cap| raw.min(cap)).unwrap_or(raw)
        }
        DiscountType::Fixed => coupon.value.min(subtotal),
        DiscountType::FreeShipping => 0,
    };

    Ok(AppliedDiscount {
        free_shipping: coupon.discount_type == DiscountType::FreeShipping,
        coupon,
        discount,
    })
}
