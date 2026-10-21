use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/product")
            .route("/list", web::get().to(Handler::Product::List::task))
            .route("/brand/list", web::get().to(Handler::Product::BrandList::task))
            .route("/{slug}", web::get().to(Handler::Product::Detail::task)),
    );
}
