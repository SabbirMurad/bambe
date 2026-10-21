use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/wishlist")
            .route("", web::get().to(Handler::Wishlist::List::task))
            .route("", web::post().to(Handler::Wishlist::Add::task))
            .route("/{id}", web::delete().to(Handler::Wishlist::Remove::task)),
    );
}
