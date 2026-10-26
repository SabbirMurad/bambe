use chrono::{Utc, TimeZone, Datelike};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Order::Order;
use crate::model::Product::{Product, ProductVariant};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // 7d | 30d (default) | 90d | ytd
    period: Option<String>,
}

fn period_start(period: &str, now: i64) -> i64 {
    match period {
        "7d" => now - 7 * DAY_MS,
        "90d" => now - 90 * DAY_MS,
        "ytd" => {
            let year = Utc.timestamp_millis_opt(now).unwrap().year();
            Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap().timestamp_millis()
        }
        _ => now - 30 * DAY_MS,
    }
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let now = Utc::now().timestamp_millis();
    let from = period_start(query.period.as_deref().unwrap_or("30d"), now);

    let order_col = db.collection::<Order>("order");
    let in_period = doc! { "placed_at": { "$gte": from, "$lte": now } };

    let mut revenue_filter = in_period.clone();
    revenue_filter.insert("payment_status", doc! { "$in": ["Paid", "PartiallyRefunded", "Refunded"] });

    let mut order_cursor = order_col.find(revenue_filter).await.expect("order lookup");
    let mut revenue: i64 = 0;
    let mut order_count: i64 = 0;
    while let Some(result) = order_cursor.next().await {
        let o = match result {
            Ok(o) => o,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        revenue += o.totals.grand_total;
        order_count += 1;
    }
    let aov = if order_count > 0 { revenue / order_count } else { 0 };

    let refund_pipeline = vec![
        doc! { "$match": { "created_at": { "$gte": from, "$lte": now }, "kind": "Refund", "status": "Succeeded" } },
        doc! { "$group": { "_id": null, "total": { "$sum": "$amount" } } },
    ];
    let refunded: i64 = match db.collection::<Document>("payment_transaction").aggregate(refund_pipeline).await {
        Ok(mut cursor) => cursor.next().await.and_then(|r| r.ok()).and_then(|d| d.get_i32("total").ok().map(|v| v as i64)).unwrap_or(0),
        Err(_) => 0,
    };
    let refund_rate = if revenue > 0 { refunded as f64 / revenue as f64 * 100.0 } else { 0.0 };

    // Day-by-day revenue for the chart — bucketed in application code rather
    // than a $dateTrunc pipeline so it works the same on any Mongo version.
    let mut daily: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    let mut cursor = order_col.find(doc! {
        "placed_at": { "$gte": from, "$lte": now },
        "payment_status": { "$in": ["Paid", "PartiallyRefunded", "Refunded"] },
    }).await.expect("order lookup");
    while let Some(result) = cursor.next().await {
        let o = match result {
            Ok(o) => o,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        let day = Utc.timestamp_millis_opt(o.placed_at).unwrap().format("%Y-%m-%d").to_string();
        *daily.entry(day).or_insert(0) += o.totals.grand_total;
    }
    let revenue_series: Vec<_> = daily.into_iter().map(|(day, amount)| json!({ "date": day, "revenue": amount })).collect();

    let mut recent_cursor = order_col.find(doc! {}).sort(doc! { "placed_at": -1 }).limit(10).await.expect("order lookup");
    let mut recent_orders = Vec::new();
    while let Some(result) = recent_cursor.next().await {
        let o = match result {
            Ok(o) => o,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        recent_orders.push(json!({
            "uuid": o.uuid,
            "number": o.number,
            "fulfilment_status": o.fulfilment_status,
            "payment_status": o.payment_status,
            "grand_total": o.totals.grand_total,
            "placed_at": o.placed_at,
        }));
    }

    let mut low_stock_cursor = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "$expr": { "$and": [{ "$gt": ["$stock", 0] }, { "$lte": ["$stock", "$low_stock_threshold"] }] } })
        .sort(doc! { "stock": 1 })
        .limit(10)
        .await
        .expect("variant lookup");
    let mut low_stock = Vec::new();
    while let Some(result) = low_stock_cursor.next().await {
        let v = match result {
            Ok(v) => v,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        let product = db.collection::<Product>("product").find_one(doc! { "uuid": &v.product_id }).await.ok().flatten();
        low_stock.push(json!({
            "variant_id": v.uuid,
            "product_id": v.product_id,
            "product_title": product.map(|p| p.title),
            "sku": v.sku,
            "variant_label": v.label(),
            "stock": v.stock,
            "low_stock_threshold": v.low_stock_threshold,
        }));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "period": query.period.clone().unwrap_or_else(|| "30d".to_string()),
        "revenue": revenue,
        "orders": order_count,
        "average_order_value": aov,
        "refund_rate": refund_rate,
        "revenue_series": revenue_series,
        "recent_orders": recent_orders,
        "low_stock": low_stock,
    })))
}
