use actix_web::web;
use crate::Handler;

/// Admin dashboard API. Every handler enforces Role(Administrator) itself.
///
/// This is the shell that survived the scaffolding: staff identity and the
/// account-deletion queue. The catalogue, order, coupon and support endpoints
/// the console needs are still to be written against the new models.
pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/admin")
            .route("/me", web::get().to(Handler::Admin::Me::task))
            .route("/admins", web::get().to(Handler::Admin::Admins::list))
            .route("/deletion-requests", web::get().to(Handler::Admin::DeletionRequests::list))
            .route("/deletion-requests/{id}/delete", web::post().to(Handler::Admin::DeletionRequests::delete))
            .route("/deletion-requests/{id}/dismiss", web::post().to(Handler::Admin::DeletionRequests::dismiss)),
    );
}
