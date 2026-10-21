use crate::BuiltIns::jwt;
use crate::BuiltIns::mongo::MongoDB;
use crate::Model::Account::{AccountCore, AccountRole, AdminAccount};
use crate::util::response::Response;
use actix_web::{Error, HttpResponse, web};
use chrono::Utc;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serde_json::json;

/*
  Only the token is read.

  `user_id` and `role` used to be fields here, and the handler minted an access
  token straight from them - so anyone holding any valid refresh token could ask
  for an access token for any account, with any role, including Administrator.
  That was full account takeover from a single request.

  They are gone. Serde ignores unknown fields by default, so a client still
  sending them keeps working; the values are simply never read.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqBody {
    refresh_token: String,
}

pub async fn task(form_data: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let refresh_token = form_data.refresh_token.trim().to_string();

    /*
      Rotate first, and mint nothing if it fails.

      `rotate` verifies the signature and expiry, confirms the token is the
      Active one on record, checks the row's issuer matches the token subject,
      and swaps in a replacement - so a refresh token is single-use. It returns
      the identity it proved, which is the only identity this handler trusts.

      The client MUST store the returned `refresh_token`; the one it sent is dead
      from here on.
    */
    let (user_id, rotated_token) = match jwt::refresh_token::rotate(&refresh_token) {
        Ok(pair) => pair,
        Err(error) => {
            log::warn!("refresh rejected: {}", error);
            // Deliberately generic: which of signature / expiry / revocation
            // failed is not the caller's business.
            return Ok(Response::unauthorized("Invalid or expired refresh token"));
        }
    };

    /*
      The role comes from the database, not the request. Shoppers live in
      `account_core` and staff in `admin_account`; a refresh token's subject is
      the uuid of whichever one signed in. Reading the account also lets a
      suspension take effect on refresh - otherwise a suspended user keeps
      minting access tokens until their refresh token expires.
    */
    let db = MongoDB.connect();
    let role = match db
        .collection::<AccountCore>("account_core")
        .find_one(doc! { "uuid": &user_id })
        .await
    {
        Ok(Some(account)) => {
            if account.suspended_at.is_some() {
                return Ok(Response::forbidden("This account has been suspended"));
            }
            account.role
        }
        Ok(None) => match db
            .collection::<AdminAccount>("admin_account")
            .find_one(doc! { "uuid": &user_id })
            .await
        {
            Ok(Some(_)) => AccountRole::Administrator,
            Ok(None) => {
                log::warn!("refresh for unknown account {}", user_id);
                return Ok(Response::unauthorized("Invalid or expired refresh token"));
            }
            Err(error) => return Ok(Response::internal_server_error(&format!("{:?}", error))),
        },
        Err(error) => return Ok(Response::internal_server_error(&format!("{:?}", error))),
    };

    let (access_token, time_in_minutes) = match jwt::access_token::generate_default(&user_id, role.clone()) {
        Ok(pair) => pair,
        Err(error) => {
            return Ok(Response::internal_server_error(&format!("could not mint an access token: {:?}", error)));
        }
    };

    let access_token_valid_till = Utc::now().timestamp_millis() + (time_in_minutes * 60 * 1000) as i64;

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "access_token": access_token,
        "access_token_valid_till": access_token_valid_till,
        // The replacement: the token that was sent is no longer valid.
        "refresh_token": rotated_token,
        "user_id": user_id,
        "role": role,
    })))
}
