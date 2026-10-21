use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/return")
            .route("/list", web::get().to(Handler::ReturnRequest::List::task))
            .route("", web::post().to(Handler::ReturnRequest::Create::task)),
    );
}
