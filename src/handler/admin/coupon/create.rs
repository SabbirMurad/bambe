use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Coupon::{Coupon, DiscountType};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    code: String,
    description: String,
    discount_type: DiscountType,
    value: i64,
    max_discount: Option<Cents>,
    min_spend: Option<Cents>,
    total_limit: Option<u32>,
    per_user_limit: Option<u32>,
    starts_at: i64,
    ends_at: Option<i64>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    product_ids: Vec<String>,
    #[serde(default)]
    category_ids: Vec<String>,
    #[serde(default)]
    first_order_only: bool,
}
fn default_true() -> bool { true }


/// `value` means a different thing per discount type, so it needs a different
/// bound per type. Nothing checked it before: a percentage over 10000 bps, or
/// any negative value, produces a discount larger than the basket and
/// `place_order` has no floor on `grand_total` — so the order goes through with
/// a negative total.
pub fn validate(discount_type: &DiscountType, value: i64, min_spend: Option<Cents>, max_discount: Option<Cents>) -> Result<(), String> {
    match discount_type {
        DiscountType::Percentage => {
            if !(1..=10_000).contains(&value) {
                return Err("A percentage discount must be between 1 and 10000 basis points (0.01% to 100%)".to_string());
            }
        }
        DiscountType::Fixed => {
            if value <= 0 {
                return Err("A fixed discount must be greater than zero".to_string());
            }
        }
        // `value` is ignored for free shipping, so there is nothing to bound.
        DiscountType::FreeShipping => {}
    }

    if let Some(min_spend) = min_spend {
        if min_spend < 0 {
            return Err("Minimum spend can't be negative".to_string());
        }
    }
    if let Some(max_discount) = max_discount {
        if max_discount <= 0 {
            return Err("Maximum discount must be greater than zero".to_string());
        }
    }

    Ok(())
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Coupon>("coupon");

    let code = body.code.trim().to_uppercase();
    if code.is_empty() {
        return Ok(Response::bad_request("Code is required"));
    }

    if let Err(message) = validate(&body.discount_type, body.value, body.min_spend, body.max_discount) {
        return Ok(Response::bad_request(&message));
    }

    if col.count_documents(doc! { "code": &code }).await.unwrap_or(0) > 0 {
        return Ok(Response::conflict("A coupon with this code already exists"));
    }

    let now = Utc::now().timestamp_millis();
    let coupon = Coupon {
        uuid: Uuid::now_v7().to_string(),
        code,
        description: body.description.clone(),
        discount_type: body.discount_type.clone(),
        value: body.value,
        max_discount: body.max_discount,
        min_spend: body.min_spend,
        total_limit: body.total_limit,
        per_user_limit: body.per_user_limit,
        used_count: 0,
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        enabled: body.enabled,
        product_ids: body.product_ids.clone(),
        category_ids: body.category_ids.clone(),
        first_order_only: body.first_order_only,
        user_id: None,
        created_at: now,
        created_by: admin.user_id,
        modified_at: now,
    };

    if let Err(error) = col.insert_one(&coupon).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &coupon.created_by).await;
    crate::Handler::Admin::Settings::AuditLog::log(&db, &coupon.created_by, &name, "coupon.created", "coupon", &coupon.uuid, Some(coupon.code.clone())).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(coupon))
}
