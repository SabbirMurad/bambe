use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/payment-method")
            .route("/list", web::get().to(Handler::PaymentMethod::List::task))
            .route("", web::post().to(Handler::PaymentMethod::Create::task))
            .route("/{id}", web::delete().to(Handler::PaymentMethod::Remove::task))
            .route("/{id}/default", web::post().to(Handler::PaymentMethod::SetDefault::task)),
    );
}
