use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/audio")
        .app_data(web::PayloadConfig::new(64 * 1024 * 1024)) // 64 MB
        .route(
            "",
            web::post().to(Handler::Audio::Upload::task)
        )
        .route(
            "/{audio_id}",
            web::get().to(Handler::Audio::Serve::task)
        )
    );
}
