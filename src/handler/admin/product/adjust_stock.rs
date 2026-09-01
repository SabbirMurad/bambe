use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
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

    // Atomic `$inc` rather than read-then-`$set`, so a checkout landing between
    // the read above and this write isn't silently undone. A write-off is also
    // guarded on there being enough to write off, which a `$set` couldn't do —
    // otherwise two concurrent -5s against 8 units leave the variant at -2.
    let mut filter = doc! { "uuid": &variant_id };
    if body.delta < 0 {
        filter.insert("stock", doc! { "$gte": -body.delta });
    }

    let updated = col
        .find_one_and_update(
            filter,
            doc! { "$inc": { "stock": body.delta }, "$set": { "modified_at": now } },
        )
        .return_document(ReturnDocument::After)
        .await;

    let new_stock = match updated {
        Ok(Some(v)) => v.stock,
        Ok(None) => return Ok(Response::bad_request("Not enough stock to remove")),
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    };

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
