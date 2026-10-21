use actix_cors::Cors;
use std::env;

/*
  Cross-Origin Resource Sharing (CORS)
  https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS

  Allowed browser origins are configured via the `CORS_ALLOWED_ORIGINS`
  environment variable (a comma-separated list) instead of being hardcoded.
  Credentialed cross-origin requests (cookies) are enabled ONLY for those
  explicit origins — the policy never combines credentials with a
  reflect-any-origin rule, which would let any site drive authenticated calls.

  The mobile app is native (not subject to CORS) and the dashboard is served
  same-origin, so a typical deployment sets `CORS_ALLOWED_ORIGINS` to the
  dashboard's own origin (e.g. `https://app.example.com`). In `development`,
  when nothing is configured, any origin is allowed but WITHOUT credentials, so a
  cross-origin page can never ride the dashboard's auth cookie.
*/
pub fn get_policy() -> Cors {
    let mode = env::var("APP_STAGE").expect("APP_STAGE must be set on .env file");
    let configured = env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();

    let mut cors = Cors::default()
        .allow_any_method()
        .allow_any_header()
        .max_age(3600);

    let mut has_origin = false;
    for origin in configured.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        cors = cors.allowed_origin(origin);
        has_origin = true;
    }

    if has_origin {
        // Specific, trusted origins only — safe to allow credentials (cookies).
        cors = cors.supports_credentials();
    } else if mode == "development" {
        // Dev convenience: reflect any origin, but WITHOUT credentials, so a
        // cross-origin request can't carry the auth cookie. Same-origin dashboard
        // requests are unaffected (they always send their own cookies).
        cors = cors.allow_any_origin();
    } else {
        log::warn!(
            "CORS_ALLOWED_ORIGINS is empty and APP_STAGE != \"development\": all \
             cross-origin requests (including same-origin dashboard POSTs, which \
             carry an Origin header) will be blocked. Set CORS_ALLOWED_ORIGINS to \
             the dashboard's origin."
        );
    }

    cors
}
