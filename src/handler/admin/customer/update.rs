use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Customer::CustomerProfile;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    note: Option<String>,
    tags: Option<Vec<String>>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let user_id = path.into_inner();
    let now = Utc::now().timestamp_millis();

    // $set and $setOnInsert can't both touch the same field on an upsert
    // insert, so `tags` only ever lands in one of the two documents below.
    let mut set_doc = doc! { "modified_at": now };
    if let Some(v) = &body.note { set_doc.insert("note", v); }
    if let Some(v) = &body.tags { set_doc.insert("tags", v); }

    let mut on_insert = doc! {
        "uuid": Uuid::now_v7().to_string(),
        "user_id": &user_id,
        "segment": "New",
        "order_count": 0,
        "lifetime_spend": 0,
        "loyalty_points": 0i64,
        "accepts_marketing": false,
        "created_at": now,
    };
    if body.tags.is_none() {
        on_insert.insert("tags", Vec::<String>::new());
    }

    let result = db
        .collection::<CustomerProfile>("customer_profile")
        .update_one(
            doc! { "user_id": &user_id },
            doc! { "$set": set_doc, "$setOnInsert": on_insert },
        )
        .upsert(true)
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(Response::ok_message("Customer updated"))
}
