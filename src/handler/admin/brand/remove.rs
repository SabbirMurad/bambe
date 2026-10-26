use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Product::Brand;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Catalogue).await?;
    let db = MongoDB.connect();
    let col = db.collection::<Brand>("brand");
    let brand_id = path.into_inner();

    let brand = match col.find_one(doc! { "uuid": &brand_id }).await.ok().flatten() {
        Some(b) => b,
        None => return Ok(Response::not_found("Brand not found")),
    };

    // A product left pointing at a deleted brand would lose its brand line
    // on the product page and drop out of the brand filter without a word.
    let in_use = super::product_counts(&db).await.get(&brand_id).copied().unwrap_or(0);
    if in_use > 0 {
        return Ok(Response::bad_request(&format!(
            "{} {} still {} this brand. Change their brand first.",
            in_use,
            if in_use == 1 { "product" } else { "products" },
            if in_use == 1 { "uses" } else { "use" },
        )));
    }

    if let Err(error) = col.delete_one(doc! { "uuid": &brand_id }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(&db, &admin.user_id, &name, "brand.deleted", "brand", &brand_id, Some(brand.name)).await;

    Ok(Response::ok_message("Brand deleted"))
}
