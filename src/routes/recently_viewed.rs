use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/recently-viewed")
            .route("", web::get().to(Handler::RecentlyViewed::List::task))
            .route("", web::post().to(Handler::RecentlyViewed::Record::task)),
    );
}
