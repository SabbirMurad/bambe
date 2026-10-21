use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/loyalty")
            .route("/cards", web::get().to(Handler::Loyalty::Cards::task))
            .route("/card/{id}/seen", web::post().to(Handler::Loyalty::Seen::task))
            .route("/card/{id}/redeem", web::post().to(Handler::Loyalty::Redeem::task)),
    );
}
