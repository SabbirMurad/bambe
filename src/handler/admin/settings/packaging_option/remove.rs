use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Order::PackagingOption;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::Shipping).await?;
    let db = MongoDB.connect();
    let id = path.into_inner();

    

    let result = db.collection::<PackagingOption>("packaging_option").delete_one(doc! { "uuid": &id }).await;
    match result {
        Ok(r) if r.deleted_count == 0 => Ok(Response::not_found("Packaging option not found")),
        Ok(_) => Ok(Response::ok_message("Packaging option deleted")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
