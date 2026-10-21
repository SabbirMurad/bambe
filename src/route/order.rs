use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/order")
            .route("/list", web::get().to(Handler::Order::List::task))
            .route("/{id}", web::get().to(Handler::Order::Detail::task))
            .route("/{id}/cancel", web::post().to(Handler::Order::Cancel::task))
            .route("/{id}/reorder", web::post().to(Handler::Order::Reorder::task)),
    );
}
