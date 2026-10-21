use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/category")
            .route("/list", web::get().to(Handler::Category::List::task)),
    );
}
