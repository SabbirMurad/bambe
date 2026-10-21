use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Order::DeliveryMethod;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // The cart subtotal, so `free_above` can be resolved into an
    // `effective_price` the checkout page can show directly.
    subtotal: Option<i64>,
}

pub async fn task(query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let result = db
        .collection::<DeliveryMethod>("delivery_method")
        .find(doc! { "enabled": true })
        .sort(doc! { "position": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut methods = Vec::new();
    while let Some(result) = cursor.next().await {
        let m = match result {
            Ok(m) => m,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        let waived = match (m.free_above, query.subtotal) {
            (Some(threshold), Some(subtotal)) => subtotal >= threshold,
            _ => false,
        };
        methods.push(json!({
            "uuid": m.uuid,
            "name": m.name,
            "description": m.description,
            "price": m.price,
            "effective_price": if waived { 0 } else { m.price },
            "free_above": m.free_above,
            "min_days": m.min_days,
            "max_days": m.max_days,
            "cutoff_minutes": m.cutoff_minutes,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(methods))
}
