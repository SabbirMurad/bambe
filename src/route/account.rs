use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/account")
            .route("/fcm-token/add", web::post().to(Handler::Account::AddFcmToken::task))
            .route("/delete", web::post().to(Handler::Account::RequestDeletion::task))
            .route("/profile", web::get().to(Handler::Account::GetProfile::task))
            .route("/profile", web::patch().to(Handler::Account::UpdateProfile::task))
            .route("/two-factor", web::patch().to(Handler::Account::TwoFactor::task))
            .route("/stats", web::get().to(Handler::Account::Stats::task)),
    );
}
