use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/address")
            .route("/list", web::get().to(Handler::Address::List::task))
            .route("", web::post().to(Handler::Address::Create::task))
            .route("/{id}", web::patch().to(Handler::Address::Update::task))
            .route("/{id}", web::delete().to(Handler::Address::Remove::task))
            .route("/{id}/default", web::post().to(Handler::Address::SetDefault::task)),
    );
}
