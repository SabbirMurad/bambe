use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Product::Brand;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: String,
    logo_image_id: Option<String>,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Catalogue).await?;
    let db = MongoDB.connect();
    let col = db.collection::<Brand>("brand");

    let name = body.name.trim().to_string();
    let slug = super::slugify(&name);
    if name.is_empty() || slug.is_empty() {
        return Ok(Response::bad_request("The brand needs a name"));
    }
    // The slug is the name with case and punctuation folded away, so a
    // matching slug is the same brand typed differently ("L'Oreal" and
    // "l oreal") — refused rather than listed twice.
    if col.count_documents(doc! { "slug": &slug }).await.unwrap_or(0) > 0 {
        return Ok(Response::conflict("A brand with this name already exists"));
    }

    let brand = Brand {
        uuid: Uuid::now_v7().to_string(),
        name,
        slug,
        logo_image_id: body.logo_image_id.clone().filter(|id| !id.is_empty()),
        created_at: Utc::now().timestamp_millis(),
    };

    if let Err(error) = col.insert_one(&brand).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let admin_name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(&db, &admin.user_id, &admin_name, "brand.created", "brand", &brand.uuid, Some(brand.name.clone())).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(brand))
}
