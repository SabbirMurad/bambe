use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Category::Category;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: String,
    slug: String,
    description: Option<String>,
    parent_id: Option<String>,
    swatch: Option<String>,
    image_id: Option<String>,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default)]
    position: u16,
}
fn default_true() -> bool { true }

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Category>("category");

    if body.name.trim().is_empty() || body.slug.trim().is_empty() {
        return Ok(Response::bad_request("Name and slug are required"));
    }

    if col.count_documents(doc! { "slug": &body.slug }).await.unwrap_or(0) > 0 {
        return Ok(Response::conflict("A category with this slug already exists"));
    }

    let now = Utc::now().timestamp_millis();
    let category = Category {
        uuid: Uuid::now_v7().to_string(),
        name: body.name.clone(),
        slug: body.slug.clone(),
        description: body.description.clone(),
        parent_id: body.parent_id.clone(),
        swatch: body.swatch.clone(),
        image_id: body.image_id.clone(),
        visible: body.visible,
        position: body.position,
        product_count: 0,
        created_at: now,
        modified_at: now,
        modified_by: Some(admin.user_id),
    };

    if let Err(error) = col.insert_one(&category).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(category))
}
