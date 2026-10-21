use chrono::Utc;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Product::Product;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let product_id = path.into_inner();

    let result = db
        .collection::<Product>("product")
        .update_one(
            doc! { "uuid": &product_id, "deleted_at": null },
            doc! { "$set": { "deleted_at": Utc::now().timestamp_millis(), "status": "Archived" } },
        )
        .await;

    match result {
        Ok(r) if r.matched_count == 0 => Ok(Response::not_found("Product not found")),
        Ok(_) => {
            crate::Handler::Admin::Category::recompute_counts(&db).await;
            let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
            crate::Handler::Admin::Settings::AuditLog::log(&db, &admin.user_id, &name, "product.deleted", "product", &product_id, None).await;
            Ok(Response::ok_message("Product deleted"))
        }
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
