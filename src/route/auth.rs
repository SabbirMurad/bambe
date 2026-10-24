use actix_web::web;
use crate::Handler;

pub fn router(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/auth")
        .route(
            "/refresh",
            web::post().to(Handler::Auth::Refresh::task)
        )
        .route(
            "/sign-up",
            web::post().to(Handler::Auth::SignUp::task)
        )
        .route(
            "/admin/sign-up",
            web::post().to(Handler::Auth::AdminSignUp::task)
        )
        .route(
            "/resend-verification-code",
            web::post().to(Handler::Auth::ResendVerificationCode::task)
        )
        .route(
            "/validate-email",
            web::post().to(Handler::Auth::VerifyEmail::task)
        )
        .route(
            "/sign-in",
            web::post().to(Handler::Auth::SignIn::task)
        )
        .route(
            "/verify-sign-in-code",
            web::post().to(Handler::Auth::VerifySignInCode::task)
        )
        .route(
            "/admin/sign-in",
            web::post().to(Handler::Auth::AdminSignIn::task)
        )
        .route(
            "/sign-out",
            web::post().to(Handler::Auth::SignOut::task)
        )
        .route(
            "/forgot-password",
            web::post().to(Handler::Auth::ForgotPassword::task)
        )
        .route(
            "/verify-reset-code",
            web::post().to(Handler::Auth::VerifyResetCode::task)
        )
        .route(
            "/reset-password",
            web::post().to(Handler::Auth::ResetPassword::task)
        )
        .route(
            "/change-password",
            web::patch().to(Handler::Auth::ChangePassword::task)
        )
        .route(
            "/social-login",
            web::post().to(Handler::Auth::SocialLogin::task)
        )
        .route(
            "/firebase-config",
            web::get().to(Handler::Auth::SocialLogin::web_config)
        )
    );

    // The frontend hardcodes the unversioned `/api/auth/refresh` URL
    // (custom_http.dart) outside the /api/v1 prefix, so keep it answerable here.
    cfg.service(
        web::scope("/api/auth")
        .route(
            "/refresh",
            web::post().to(Handler::Auth::Refresh::task)
        )
    );
}