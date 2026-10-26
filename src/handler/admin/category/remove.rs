use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Category::Category;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Catalogue).await?;
    let db = MongoDB.connect();
    let col = db.collection::<Category>("category");
    let category_id = path.into_inner();

    let category = col.find_one(doc! { "uuid": &category_id }).await.ok().flatten();
    let category = match category {
        Some(c) => c,
        None => return Ok(Response::not_found("Category not found")),
    };

    if category.product_count > 0 {
        return Ok(Response::bad_request("Move or remove its products before deleting this category"));
    }

    let has_children = col.count_documents(doc! { "parent_id": &category_id }).await.unwrap_or(0) > 0;
    if has_children {
        return Ok(Response::bad_request("Delete or move its subcategories first"));
    }

    if let Err(error) = col.delete_one(doc! { "uuid": &category_id }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(&db, &admin.user_id, &name, "category.deleted", "category", &category_id, Some(category.name)).await;

    Ok(Response::ok_message("Category deleted"))
}
