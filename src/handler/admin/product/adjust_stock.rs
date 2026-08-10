use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Product::{ProductVariant, StockMovement, StockReason};
use super::recompute_aggregates;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    // Signed: positive restocks, negative writes off damage/loss.
    delta: i32,
    reason: StockReason,
    note: Option<String>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<ProductVariant>("product_variant");
    let variant_id = path.into_inner();

    let variant = col.find_one(doc! { "uuid": &variant_id }).await.ok().flatten();
    let variant = match variant {
        Some(v) => v,
        None => return Ok(Response::not_found("Variant not found")),
    };

    if body.delta == 0 {
        return Ok(Response::bad_request("delta can't be zero"));
    }

    let now = Utc::now().timestamp_millis();
    let new_stock = variant.stock + body.delta;

    let result = col
        .update_one(doc! { "uuid": &variant_id }, doc! { "$set": { "stock": new_stock, "modified_at": now } })
        .await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let movement = StockMovement {
        uuid: Uuid::now_v7().to_string(),
        variant_id: variant_id.clone(),
        delta: body.delta,
        balance_after: new_stock,
        reason: body.reason.clone(),
        order_id: None,
        note: body.note.clone(),
        created_by: Some(admin.user_id),
        created_at: now,
    };
    if let Err(error) = db.collection::<StockMovement>("stock_movement").insert_one(&movement).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    recompute_aggregates(&db, &variant.product_id).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(movement))
}
