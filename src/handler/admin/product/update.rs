use chrono::Utc;
use serde::Deserialize;
use mongodb::bson::{doc, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Product::{Product, ProductStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    title: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    brand_id: Option<String>,
    category_id: Option<String>,
    tags: Option<Vec<String>>,
    status: Option<ProductStatus>,
    compare_at_price: Option<Cents>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Product>("product");
    let product_id = path.into_inner();

    if col.find_one(doc! { "uuid": &product_id, "deleted_at": null }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Product not found"));
    }

    let mut set_doc = Document::new();
    if let Some(v) = &body.title { set_doc.insert("title", v); }
    if let Some(v) = &body.slug { set_doc.insert("slug", v); }
    if let Some(v) = &body.description { set_doc.insert("description", v); }
    if let Some(v) = &body.brand_id { set_doc.insert("brand_id", v); }
    if let Some(v) = &body.category_id { set_doc.insert("category_id", v); }
    if let Some(v) = &body.tags { set_doc.insert("tags", v); }
    // No `images` here: images belong to the variants. The cover is
    // denormalised from them by recompute_aggregates, never set directly.
    if let Some(v) = &body.status { set_doc.insert("status", mongodb::bson::to_bson(v).unwrap()); }
    if let Some(v) = body.compare_at_price { set_doc.insert("compare_at_price", v); }
    set_doc.insert("modified_at", Utc::now().timestamp_millis());

    let result = col.update_one(doc! { "uuid": &product_id }, doc! { "$set": set_doc }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    if body.category_id.is_some() {
        crate::Handler::Admin::Category::recompute_counts(&db).await;
    }

    let updated = col.find_one(doc! { "uuid": &product_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
