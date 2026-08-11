use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Payment::PaymentMethod;

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let result = db
        .collection::<PaymentMethod>("payment_method")
        .find(doc! { "user_id": &user.user_id, "deleted_at": null })
        .sort(doc! { "is_default": -1, "created_at": -1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut methods = Vec::new();
    // provider_token never leaves the server — it's the one thing that can
    // actually charge this method.
    while let Some(Ok(m)) = cursor.next().await {
        methods.push(json!({
            "uuid": m.uuid,
            "kind": m.kind,
            "provider": m.provider,
            "card_brand": m.card_brand,
            "last4": m.last4,
            "exp_month": m.exp_month,
            "exp_year": m.exp_year,
            "holder_name": m.holder_name,
            "account_email": m.account_email,
            "is_default": m.is_default,
            "created_at": m.created_at,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(methods))
}
