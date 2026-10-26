use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::require_shopper;
use crate::model::Cart::WishlistItem;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let user = require_shopper(&req)?;
    let db = MongoDB.connect();

    let result = db
        .collection::<WishlistItem>("wishlist_item")
        .delete_one(doc! { "uuid": path.into_inner(), "user_id": &user.user_id })
        .await;

    match result {
        Ok(r) if r.deleted_count == 0 => Ok(Response::not_found("Item not found")),
        Ok(_) => Ok(Response::ok_message("Removed from wishlist")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}
