/*
  Project identity — the one place the app's name, support link and mail
  sender are defined.

  These used to be `const PROJECT_NAME: &str = "Fanari"` duplicated across
  builtins/jwt.rs, integrations/smtp.rs and integrations/resend.rs, which meant
  renaming a project touched three files and was easy to half-finish. Each value
  is read from the environment once and cached for the process lifetime.
*/

use std::env;
use std::sync::OnceLock;

/// Reads `key` once, falling back to `default` when unset or empty.
fn cached(cell: &'static OnceLock<String>, key: &str, default: &str) -> &'static str {
    cell.get_or_init(|| {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default.to_string())
    })
    .as_str()
}

/// Display name for the product: email bodies, the `From:` header, and the JWT
/// `iss` claim. Defaults to the crate name so a fresh clone still runs.
pub fn project_name() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    cached(&VALUE, "APP_NAME", env!("CARGO_PKG_NAME"))
}

/// Support link printed at the bottom of transactional emails.
pub fn support_url() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    cached(&VALUE, "SUPPORT_URL", "https://example.com/support")
}

/// Envelope sender for Resend, as a full `Name <address>` header value.
/// The address must be on a domain verified with your mail provider.
pub fn mail_from() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    VALUE
        .get_or_init(|| {
            let address = env::var("MAIL_FROM")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "no-reply@example.com".to_string());

            format!("{} <{}>", project_name(), address)
        })
        .as_str()
}
