use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/review")
            .route("/list", web::get().to(Handler::Review::List::task))
            .route("/eligible", web::get().to(Handler::Review::Eligible::task))
            .route("/reviewed", web::get().to(Handler::Review::Reviewed::task))
            .route("", web::post().to(Handler::Review::Create::task))
            .route("/{id}/vote", web::post().to(Handler::Review::Vote::task)),
    );
}
