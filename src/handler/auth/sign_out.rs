use actix_session::Session;
use serde::{ Serialize, Deserialize };
use crate::BuiltIns::jwt;
use crate::util::response::Response;
use actix_web::{web, Error, HttpResponse};

/// Optional body. The storefront keeps its refresh token in localStorage and
/// hands it back here; the server-side session holds a copy too, which is the
/// fallback when the body doesn't.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReqBody {
    #[serde(default)]
    refresh_token: Option<String>,
}

pub async fn task(
    body: Option<web::Json<ReqBody>>,
    actix_session: Session,
) -> Result<HttpResponse, Error> {
    /*
      Revoke the refresh token, not just the session.

      `session.purge()` alone dropped the cookie and left the refresh token fully
      valid, so signing out revoked nothing: anyone still holding the token
      (including whoever the user signed out because of) could keep minting
      access tokens until it expired.
    */
    let token = body
        .and_then(|body| body.into_inner().refresh_token)
        .filter(|token| !token.trim().is_empty())
        .or_else(|| actix_session.get::<String>("refresh_token").ok().flatten());

    if let Some(token) = token {
        match jwt::refresh_token::block(&token) {
            Ok(true) => (),
            // Not an error worth failing the sign-out over: the token may have
            // already been rotated away or revoked.
            Ok(false) => log::warn!("sign-out: refresh token was not on record"),
            Err(error) => log::error!("sign-out: could not revoke refresh token: {:?}", error),
        }
    }

    actix_session.purge();

    Ok(HttpResponse::Ok().content_type("application/json").json(
        Response { message: "Successfully Signed Out".to_string() }
    ))
}
