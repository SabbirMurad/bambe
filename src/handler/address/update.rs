use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Address::Address;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    label: Option<String>,
    recipient_name: Option<String>,
    street: Option<String>,
    city: Option<String>,
    state: Option<String>,
    zip: Option<String>,
    country: Option<String>,
    phone: Option<String>,
    note: Option<String>,
    is_default: Option<bool>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<Address>("address");
    let address_id = path.into_inner();

    let owns = col
        .find_one(doc! { "uuid": &address_id, "user_id": &user.user_id, "deleted_at": null })
        .await
        .ok()
        .flatten();
    if owns.is_none() {
        return Ok(Response::not_found("Address not found"));
    }

    if body.is_default == Some(true) {
        col.update_many(
            doc! { "user_id": &user.user_id, "deleted_at": null },
            doc! { "$set": { "is_default": false } },
        )
        .await
        .ok();
    }

    let mut set_doc = Document::new();
    macro_rules! apply {
        ($field:ident) => {
            if let Some(v) = &body.$field {
                set_doc.insert(stringify!($field), v);
            }
        };
    }
    apply!(label);
    apply!(recipient_name);
    apply!(street);
    apply!(city);
    apply!(country);
    apply!(phone);
    if let Some(state) = &body.state { set_doc.insert("state", state); }
    if let Some(zip) = &body.zip { set_doc.insert("zip", zip); }
    if let Some(note) = &body.note { set_doc.insert("note", note); }
    if let Some(is_default) = body.is_default { set_doc.insert("is_default", is_default); }
    set_doc.insert("modified_at", Utc::now().timestamp_millis());

    let result = col.update_one(doc! { "uuid": &address_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let updated = col.find_one(doc! { "uuid": &address_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
