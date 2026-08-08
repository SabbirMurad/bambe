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

pub fn require_access(
    req: &HttpRequest,
    requirement: AccessRequirement,
) -> Result<User, Error> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    if auth_header.is_none() {
        return Err(actix_web::error::ErrorUnauthorized(
            json!({ "error": "Missing authorization header" }),
        ));
    }

    let token = auth_header
        .unwrap()
        .trim_start_matches("Bearer ")
        .to_string();

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