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
            .route("/deletion-requests/{id}/dismiss", web::post().to(Handler::Admin::DeletionRequests::dismiss))
            .route("/product/list", web::get().to(Handler::Admin::Product::List::task))
            .route("/product", web::post().to(Handler::Admin::Product::Create::task))
            .route("/product/{id}", web::patch().to(Handler::Admin::Product::Update::task))
            .route("/product/{id}", web::delete().to(Handler::Admin::Product::Remove::task))
            .route("/product/{id}/variant", web::post().to(Handler::Admin::Product::AddVariant::task))
            .route("/product/variant/{id}", web::patch().to(Handler::Admin::Product::UpdateVariant::task))
            .route("/product/variant/{id}", web::delete().to(Handler::Admin::Product::RemoveVariant::task))
            .route("/product/variant/{id}/stock", web::post().to(Handler::Admin::Product::AdjustStock::task))
            .route("/category/list", web::get().to(Handler::Admin::Category::List::task))
            .route("/category", web::post().to(Handler::Admin::Category::Create::task))
            .route("/category/{id}", web::patch().to(Handler::Admin::Category::Update::task))
            .route("/category/{id}", web::delete().to(Handler::Admin::Category::Remove::task))
            .route("/order/list", web::get().to(Handler::Admin::Order::List::task))
            .route("/order/{id}", web::get().to(Handler::Admin::Order::Detail::task))
            .route("/order/{id}/status", web::patch().to(Handler::Admin::Order::UpdateStatus::task))
            .route("/order/{id}/tracking", web::post().to(Handler::Admin::Order::SetTracking::task))
            .route("/order/{id}/refund", web::post().to(Handler::Admin::Order::Refund::task))
            .route("/coupon/list", web::get().to(Handler::Admin::Coupon::List::task))
            .route("/coupon", web::post().to(Handler::Admin::Coupon::Create::task))
            .route("/coupon/{id}", web::patch().to(Handler::Admin::Coupon::Update::task))
            .route("/coupon/{id}", web::delete().to(Handler::Admin::Coupon::Remove::task))
            .route("/coupon/{id}/duplicate", web::post().to(Handler::Admin::Coupon::Duplicate::task))
            .route("/customer/list", web::get().to(Handler::Admin::Customer::List::task))
            .route("/customer/{id}", web::get().to(Handler::Admin::Customer::Detail::task))
            .route("/customer/{id}", web::patch().to(Handler::Admin::Customer::Update::task))
            .route("/support/ticket/list", web::get().to(Handler::Admin::SupportTicket::List::task))
            .route("/support/ticket/{id}", web::get().to(Handler::Admin::SupportTicket::Detail::task))
            .route("/support/ticket/{id}/message", web::post().to(Handler::Admin::SupportTicket::Reply::task))
            .route("/support/ticket/{id}/assign", web::post().to(Handler::Admin::SupportTicket::Assign::task))
            .route("/support/ticket/{id}/status", web::patch().to(Handler::Admin::SupportTicket::SetStatus::task))
            .route("/settings/store", web::get().to(Handler::Admin::Settings::GetStore::task))
            .route("/settings/store", web::patch().to(Handler::Admin::Settings::UpdateStore::task))
            .route("/settings/shipping-zone/list", web::get().to(Handler::Admin::Settings::ShippingZone::List::task))
            .route("/settings/shipping-zone", web::post().to(Handler::Admin::Settings::ShippingZone::Create::task))
            .route("/settings/shipping-zone/{id}", web::patch().to(Handler::Admin::Settings::ShippingZone::Update::task))
            .route("/settings/shipping-zone/{id}", web::delete().to(Handler::Admin::Settings::ShippingZone::Remove::task))
            .route("/settings/staff/list", web::get().to(Handler::Admin::Settings::Staff::List::task))
            .route("/settings/staff/invite", web::post().to(Handler::Admin::Settings::Staff::Invite::task))
            .route("/settings/staff/{id}", web::patch().to(Handler::Admin::Settings::Staff::Update::task))
            .route("/settings/audit-log/list", web::get().to(Handler::Admin::Settings::AuditLog::task))
            .route("/dashboard", web::get().to(Handler::Admin::Dashboard::task)),
    );
}
