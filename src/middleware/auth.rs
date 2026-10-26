use serde_json::json;
use crate::BuiltIns::jwt;
use crate::Model::Account::AccountRole;
use actix_web::{ Error, HttpRequest};

#[derive(Debug, Clone)]
pub enum AccessRequirement {
    AnyToken,
    Role(AccountRole),
    AnyOf(Vec<AccountRole>),
}

#[derive(Debug)]
pub struct User {
    pub user_id: String,
    pub role: AccountRole,
}

/// Parse an `Authorization: Bearer <token>` credential (RFC 6750 §2.1).
///
/// The scheme is matched case-insensitively because RFC 7235 §2.1 defines it as
/// a case-insensitive token — `bearer abc` is as valid as `Bearer abc`.
///
/// Deliberately strict about what it rejects:
///   - a header with no scheme at all, so a bare token is not mistaken for one
///   - any scheme other than Bearer (`Basic dXNlcjpwYXNz` is not a token)
///   - a value with internal whitespace, which would mean a second credential
///     was appended — the token68 grammar has no spaces in it
///
/// This replaces `trim_start_matches("Bearer ")`, which stripped the prefix
/// *repeatedly* (`Bearer Bearer x` → `x`) and silently fell through to using the
/// whole header value as the token when no prefix was present.
fn bearer_token(header: &str) -> Option<String> {
    let (scheme, credentials) = header.trim().split_once(' ')?;

    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }

    let token = credentials.trim();
    if token.is_empty() || token.contains(char::is_whitespace) {
        return None;
    }

    Some(token.to_string())
}

/// The access token on the request, from its Bearer header. Header only: the
/// storefront and console both send it that way, and accepting a cookie as
/// well would make every API call ride along on cross-site requests.
fn extract_token(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bearer_token)
}

pub fn require_access(
    req: &HttpRequest,
    requirement: AccessRequirement,
) -> Result<User, Error> {
    let token = match extract_token(req) {
        Some(t) => t,
        None => {
            return Err(actix_web::error::ErrorUnauthorized(
                json!({ "error": "Missing authorization header" }),
            ));
        }
    };

    // Validate access token
    let claims = jwt::access_token::verify(&token, jwt::Key::Local)
        .map_err(|err| {
            log::error!("{:?}", err);
            actix_web::error::ErrorUnauthorized(
                json!({ "error": "Invalid authorization token" }),
            )
        })?;

    let pass = match &requirement {
        AccessRequirement::AnyToken => true,
        AccessRequirement::Role(r) => &claims.role == r,
        AccessRequirement::AnyOf(roles) => roles.contains(&claims.role),
    };

    if !pass {
        return Err(actix_web::error::ErrorForbidden(
            json!({ "error": "Not authorized to perform this action" }),
        ));
    }

    Ok(User {
        user_id: claims.sub,
        role: claims.role,
    })
}

/*
  The cart, wishlist, checkout and orders belong to customer accounts.
  An admin who filled a cart or placed an order would land in the order
  queue and sales figures they manage, against an id that has no
  customer profile, address book or notifications behind it.

  `require_shopper` is for endpoints that need a signed-in customer;
  `refuse_staff` for the cart ones a guest may also call, where the only
  thing to rule out is a staff token.
*/
const SHOPPERS_ONLY: &str =
    "Admin accounts can't shop. Sign in with a customer account to use the cart, wishlist and checkout.";

pub fn require_shopper(req: &HttpRequest) -> Result<User, Error> {
    let user = require_access(req, AccessRequirement::AnyToken)?;
    if user.role != AccountRole::User {
        return Err(actix_web::error::ErrorForbidden(json!({ "error": SHOPPERS_ONLY })));
    }
    Ok(user)
}

pub fn refuse_staff(req: &HttpRequest) -> Result<(), Error> {
    match optional_access(req) {
        Some(user) if user.role != AccountRole::User => {
            Err(actix_web::error::ErrorForbidden(json!({ "error": SHOPPERS_ONLY })))
        }
        _ => Ok(()),
    }
}

/*
  Same as `require_access`, but for endpoints a guest may also call — the
  cart, mainly. Returns `None` on a missing or invalid token instead of
  erroring, so the caller can fall back to session-based identity.
*/
pub fn optional_access(req: &HttpRequest) -> Option<User> {
    let token = extract_token(req)?;

    let claims = jwt::access_token::verify(&token, jwt::Key::Local).ok()?;

    Some(User {
        user_id: claims.sub,
        role: claims.role,
    })
}