use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Support::SupportTicket;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // open | pending | resolved | closed — omitted means every status.
    status: Option<String>,
    priority: Option<String>,
    topic: Option<String>,
    q: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<SupportTicket>("support_ticket");

    let mut filter = doc! {};
    if let Some(status) = &query.status {
        let variant = match status.to_lowercase().as_str() {
            "pending" => "Pending",
            "resolved" => "Resolved",
            "closed" => "Closed",
            _ => "Open",
        };
        filter.insert("status", variant);
    }
    if let Some(priority) = &query.priority {
        filter.insert("priority", priority);
    }
    if let Some(topic) = &query.topic {
        filter.insert("topic", topic);
    }
    if let Some(q) = &query.q {
        filter.insert("$or", vec![
            doc! { "number": { "$regex": q, "$options": "i" } },
            doc! { "subject": { "$regex": q, "$options": "i" } },
            doc! { "email": { "$regex": q, "$options": "i" } },
            doc! { "name": { "$regex": q, "$options": "i" } },
        ]);
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let page = query.page.unwrap_or(1).max(1) as i64;

    let total = col.count_documents(filter.clone()).await.unwrap_or(0);
    let result = col.find(filter).sort(doc! { "last_message_at": -1 }).skip((limit * (page - 1)) as u64).limit(limit).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut tickets = Vec::new();
    while let Some(result) = cursor.next().await {
        let t = match result {
            Ok(t) => t,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        tickets.push(t);
    }

    let open_count = col.count_documents(doc! { "status": "Open" }).await.unwrap_or(0);
    let pending_count = col.count_documents(doc! { "status": "Pending" }).await.unwrap_or(0);
    let unread_count = col.count_documents(doc! { "unread_by_staff": true }).await.unwrap_or(0);

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "items": tickets,
        "total": total,
        "page": page,
        "limit": limit,
        "stats": { "open": open_count, "pending": pending_count, "unread": unread_count },
    })))
}
