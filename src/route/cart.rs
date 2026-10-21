use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/cart")
            .route("", web::get().to(Handler::Cart::Get::task))
            .route("/item", web::post().to(Handler::Cart::AddItem::task))
            .route("/item/{id}", web::patch().to(Handler::Cart::UpdateItem::task))
            .route("/item/{id}", web::delete().to(Handler::Cart::RemoveItem::task))
            .route("/coupon", web::post().to(Handler::Cart::ApplyCoupon::task))
            .route("/coupon", web::delete().to(Handler::Cart::RemoveCoupon::task))
            .route("/merge", web::post().to(Handler::Cart::Merge::task)),
    );
}
