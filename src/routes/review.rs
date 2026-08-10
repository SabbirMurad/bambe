use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/review")
            .route("/list", web::get().to(Handler::Review::List::task))
            .route("", web::post().to(Handler::Review::Create::task))
            .route("/{id}/vote", web::post().to(Handler::Review::Vote::task)),
    );
}
