use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Payment::{PaymentMethod, PaymentMethodKind, CardBrand};

// `provider_token` is expected to already be an opaque handle from the
// processor's client SDK (Stripe.js and friends tokenise the raw card in
// the browser) — this endpoint only ever stores that handle, never a PAN
// or CVV.
#[derive(Debug, Deserialize)]
pub struct ReqBody {
    kind: PaymentMethodKind,
    provider: String,
    provider_token: String,
    card_brand: Option<CardBrand>,
    last4: Option<String>,
    exp_month: Option<u8>,
    exp_year: Option<u16>,
    holder_name: Option<String>,
    account_email: Option<String>,
    #[serde(default)]
    is_default: bool,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<PaymentMethod>("payment_method");

    if body.provider.trim().is_empty() || body.provider_token.trim().is_empty() {
        return Ok(Response::bad_request("provider and provider_token are required"));
    }

    let has_existing = col
        .count_documents(doc! { "user_id": &user.user_id, "deleted_at": null })
        .await
        .unwrap_or(0)
        > 0;
    let is_default = body.is_default || !has_existing;

    if is_default {
        col.update_many(
            doc! { "user_id": &user.user_id, "deleted_at": null },
            doc! { "$set": { "is_default": false } },
        )
        .await
        .ok();
    }

    let method = PaymentMethod {
        uuid: Uuid::now_v7().to_string(),
        user_id: user.user_id,
        kind: body.kind.clone(),
        provider: body.provider.clone(),
        provider_token: body.provider_token.clone(),
        card_brand: body.card_brand.clone(),
        last4: body.last4.clone(),
        exp_month: body.exp_month,
        exp_year: body.exp_year,
        holder_name: body.holder_name.clone(),
        account_email: body.account_email.clone(),
        is_default,
        created_at: Utc::now().timestamp_millis(),
        deleted_at: None,
    };

    if let Err(error) = col.insert_one(&method).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(serde_json::json!({ "uuid": method.uuid })))
}
