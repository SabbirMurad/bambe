use futures::StreamExt;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Category::Category;

#[derive(Debug, Deserialize)]
pub struct ReqQuery {
    // all (default) | visible | hidden | empty
    tab: Option<String>,
}

pub async fn task(req: HttpRequest, query: web::Query<ReqQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let filter = match query.tab.as_deref() {
        Some("visible") => doc! { "visible": true },
        Some("hidden") => doc! { "visible": false },
        Some("empty") => doc! { "product_count": 0 },
        _ => doc! {},
    };

    let result = db.collection::<Category>("category").find(filter).sort(doc! { "position": 1 }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut categories = Vec::new();
    while let Some(result) = cursor.next().await {
        let c = match result {
            Ok(c) => c,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        categories.push(c);
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(categories))
}
