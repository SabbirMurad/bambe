use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/notification")
            .route("/list", web::get().to(Handler::Notification::List::task))
            .route("/read", web::patch().to(Handler::Notification::MarkRead::task))
            .route("/read-all", web::patch().to(Handler::Notification::MarkAllRead::task))
            .route("/preferences", web::get().to(Handler::Notification::GetPreferences::task))
            .route("/preferences", web::patch().to(Handler::Notification::UpdatePreferences::task)),
    );
}
