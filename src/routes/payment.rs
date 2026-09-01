use actix_web::web;
use crate::Handler;

/*
  Outside `/api` and unauthenticated, because SSLCommerz calls these, not a
  signed-in browser. Nothing in a callback body is trusted — the handlers
  re-ask the gateway over a server-to-server call before believing anything.

  `success`, `fail` and `cancel` are browser form-POSTs and answer with a
  redirect; `ipn` is server-to-server. `/abandon` is the one authenticated
  route here: it is the shopper's own "I gave up", not the gateway's.
*/
pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/payment/sslcommerz")
            .route("/success", web::post().to(Handler::Payment::SslCommerz::success))
            .route("/fail", web::post().to(Handler::Payment::SslCommerz::fail))
            .route("/cancel", web::post().to(Handler::Payment::SslCommerz::cancel))
            .route("/ipn", web::post().to(Handler::Payment::SslCommerz::ipn)),
    );
    cfg.service(
        web::scope("/api/v1/payment")
            .route("/abandon", web::post().to(Handler::Payment::SslCommerz::abandon)),
    );
}
