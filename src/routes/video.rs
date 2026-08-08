use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/video")
        .app_data(
        /*
            MAXIMUM POST DATA LIMIT: 64MB
            Since `JSON.stringify()` increase the payload to 8x
            Only 16MB of JSON data will be allowed from the client side code
        */
        web::JsonConfig::default().limit(1024 * 1024 * 128)
        )
        .route(
            "/upload",
            web::post().to(Handler::Video::Upload::task)
        )
        .route(
            "/{video_id}/video.mp4",
            web::get().to(Handler::Video::Serve::task)
        )
        .route(
            "/segment/{video_id}",
            web::get().to(Handler::Video::Index::task)
        )
        .route(
            "/segment/{video_id}/{segment_name}",
            web::get().to(Handler::Video::Segment::task)
        )
    );
}