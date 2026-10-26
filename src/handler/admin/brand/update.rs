use serde::Deserialize;
use mongodb::bson::{doc, Bson, Document};
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Product::Brand;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    name: Option<String>,
    /// Absent leaves the logo; "" removes it.
    logo_image_id: Option<String>,
}

pub async fn task(req: HttpRequest, path: web::Path<String>, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Catalogue).await?;
    let db = MongoDB.connect();
    let col = db.collection::<Brand>("brand");
    let brand_id = path.into_inner();

    if col.find_one(doc! { "uuid": &brand_id }).await.ok().flatten().is_none() {
        return Ok(Response::not_found("Brand not found"));
    }

    let mut set_doc = Document::new();
    if let Some(name) = &body.name {
        let name = name.trim();
        let slug = super::slugify(name);
        if name.is_empty() || slug.is_empty() {
            return Ok(Response::bad_request("The brand needs a name"));
        }
        let taken = col
            .count_documents(doc! { "slug": &slug, "uuid": { "$ne": &brand_id } })
            .await
            .unwrap_or(0) > 0;
        if taken {
            return Ok(Response::conflict("A brand with this name already exists"));
        }
        // Renaming re-slugs: products point at the brand by uuid, so no
        // link breaks, and the listing's brand filter goes by uuid too.
        set_doc.insert("name", name);
        set_doc.insert("slug", slug);
    }
    if let Some(logo) = &body.logo_image_id {
        set_doc.insert("logo_image_id", if logo.is_empty() { Bson::Null } else { Bson::String(logo.clone()) });
    }
    if set_doc.is_empty() {
        return Ok(Response::bad_request("Nothing to change"));
    }

    if let Err(error) = col.update_one(doc! { "uuid": &brand_id }, doc! { "$set": set_doc }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let updated = col.find_one(doc! { "uuid": &brand_id }).await.ok().flatten();
    Ok(HttpResponse::Ok().content_type("application/json").json(updated))
}
