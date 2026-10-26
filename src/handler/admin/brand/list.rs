use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Product::Brand;

/*
  Every brand, A-Z, each with how many live products carry it. Not paged:
  a shop has tens of brands, and the product form's dropdown needs them all.
*/
pub async fn task(req: HttpRequest) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let result = db.collection::<Brand>("brand").find(doc! {}).sort(doc! { "name": 1 }).await;
    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let counts = super::product_counts(&db).await;
    let mut cursor = result.unwrap();
    let mut brands = Vec::new();
    while let Some(next) = cursor.next().await {
        match next {
            Ok(b) => brands.push(json!({
                "uuid": b.uuid,
                "name": b.name,
                "slug": b.slug,
                "logo_image_id": b.logo_image_id,
                "created_at": b.created_at,
                "product_count": counts.get(&b.uuid).copied().unwrap_or(0),
            })),
            Err(error) => log::error!("{}: skipping unreadable document: {:?}", module_path!(), error),
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(brands))
}
