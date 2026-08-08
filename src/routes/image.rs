use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/image")
        // JsonConfig bounds the JSON extractor (the /metadata endpoints);
        // PayloadConfig bounds the raw multipart body of the upload so an
        // image POST can't be unbounded.
        .app_data(web::JsonConfig::default().limit(16 * 1024 * 1024))
        .app_data(web::PayloadConfig::new(64 * 1024 * 1024)) // 64 MB
        .route(
            "",
            web::post().to(Handler::Image::Upload::task)
        )
        .route(
            "/webp/{image_id}",
            web::get().to(Handler::Image::Webp::task)
        )
        .route(
            "/original/{image_id}",
            web::get().to(Handler::Image::Original::task)
        )
        .route(
            "/metadata/{image_id}", 
            web::get().to(Handler::Image::Metadata::task)
        )
        .route(
            "/metadata", 
            web::post().to(Handler::Image::metadata_bulk::task)
        )
    );
}