use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/support/ticket")
            .route("/list", web::get().to(Handler::SupportTicket::List::task))
            .route("", web::post().to(Handler::SupportTicket::Create::task))
            .route("/{id}", web::get().to(Handler::SupportTicket::Detail::task))
            .route("/{id}/message", web::post().to(Handler::SupportTicket::Reply::task)),
    );
}
