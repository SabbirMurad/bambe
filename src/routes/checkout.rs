use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/checkout")
            .route("/delivery-methods", web::get().to(Handler::Checkout::DeliveryMethods::task))
            .route("/packaging-options", web::get().to(Handler::Checkout::PackagingOptions::task))
            .route("/place-order", web::post().to(Handler::Checkout::PlaceOrder::task)),
    );
}
