use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/metadata")
        .route(
            "/update",
            web::post().to(Handler::Metadata::Update::task)
        )
        .route(
            "/get",
            web::get().to(Handler::Metadata::Get::task)
        )
    );
}