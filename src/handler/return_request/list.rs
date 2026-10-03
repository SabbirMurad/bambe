use std::collections::HashMap;
use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Order::{Order, ReturnRequest, ReturnItem};

pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();

    let result = db
        .collection::<ReturnRequest>("return_request")
        .find(doc! { "user_id": &user.user_id })
        .sort(doc! { "requested_at": -1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut returns = Vec::new();
    while let Some(result) = cursor.next().await {
        let r = match result {
            Ok(r) => r,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        returns.push(r);
    }

    let order_ids: Vec<String> = returns.iter().map(|r| r.order_id.clone()).collect();
    let orders: HashMap<String, Order> = if order_ids.is_empty() {
        HashMap::new()
    } else {
        let mut cursor = db.collection::<Order>("order").find(doc! { "uuid": { "$in": &order_ids } }).await.expect("order lookup");
        let mut map = HashMap::new();
        while let Some(result) = cursor.next().await {
            let o = match result {
                Ok(o) => o,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            map.insert(o.uuid.clone(), o);
        }
        map
    };

    let mut items = Vec::new();
    for r in &returns {
        let mut item_cursor = db
            .collection::<ReturnItem>("return_item")
            .find(doc! { "return_request_id": &r.uuid })
            .await
            .expect("return item lookup");
        let mut lines = Vec::new();
        while let Some(result) = item_cursor.next().await {
            let i = match result {
                Ok(i) => i,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            lines.push(i);
        }

        items.push(json!({
            "uuid": r.uuid,
            "order_id": r.order_id,
            "order_number": orders.get(&r.order_id).map(|o| o.number.clone()),
            "items": lines,
            "reason": r.reason,
            "details": r.details,
            "image_ids": r.image_ids,
            "status": r.status,
            "refund_amount": r.refund_amount,
            "requested_at": r.requested_at,
            "resolved_at": r.resolved_at,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(items))
}
