//! Turn two-step sign-in on or off.
//!
//! `two_a_factor_auth_enabled` has always been read by `sign_in` — when it is
//! set, sign-in stops and emails a code instead of returning a token — but
//! nothing could ever write it, so the profile page could only display the
//! state behind a switch that did nothing.
//!
//! Turning it **on** is free. Turning it **off** removes a factor from the
//! account, so it costs the current password: an attacker holding a live token
//! (a borrowed laptop, a stolen session) must not be able to quietly disable
//! the thing that would stop them signing in again.

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::util::password::{self, Verified};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountCore;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    enabled: bool,
    /// Required only when switching off.
    password: Option<String>,
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let user = require_access(&req, AccessRequirement::AnyToken)?;
    let db = MongoDB.connect();
    let col = db.collection::<AccountCore>("account_core");

    let account = col.find_one(doc! { "uuid": &user.user_id }).await.ok().flatten();
    let account = match account {
        Some(account) => account,
        None => return Ok(Response::not_found("Account not found")),
    };

    // Already there. Answering 200 rather than erroring keeps the switch
    // idempotent, which matters when a double-click sends two requests.
    if account.two_a_factor_auth_enabled == body.enabled {
        return Ok(HttpResponse::Ok().content_type("application/json").json(json!({
            "two_factor_enabled": account.two_a_factor_auth_enabled,
        })));
    }

    if !body.enabled {
        let password = match &body.password {
            Some(password) if !password.is_empty() => password,
            _ => return Ok(Response::bad_request("Your password is required to turn this off")),
        };

        /*
          `util::password` owns the comparison; this handler never sees the
          stored form. It deliberately does not upgrade a legacy row the way
          sign_in.rs does — turning off two-step is not an authentication,
          and a write here would be a side effect nobody asked for.
        */
        if password::verify(password, &account.password) == Verified::Failed {
            return Ok(Response::unauthorized("That password isn't right"));
        }
    }

    /*
      Two-step sign-in delivers its code by email, so an unverified address
      would lock the account out of itself — the code would go somewhere the
      owner has not proven they can read.
    */
    if body.enabled && !account.email_verified {
        return Ok(Response::bad_request(
            "Verify your email address before turning on two-step sign-in",
        ));
    }

    let now = Utc::now().timestamp_millis();
    let result = col
        .update_one(
            doc! { "uuid": &user.user_id },
            doc! { "$set": {
                "two_a_factor_auth_enabled": body.enabled,
                "two_a_factor_auth_updated": now,
            } },
        )
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    // A security change the owner should hear about even if they made it —
    // this is the notification that betrays an attacker who made it instead.
    crate::util::notify::send(
        &db,
        &user.user_id,
        crate::util::notify::Notice {
            kind: crate::model::Notification::NotifType::Security,
            title: if body.enabled {
                "Two-step sign-in is on".to_string()
            } else {
                "Two-step sign-in is off".to_string()
            },
            body: if body.enabled {
                "You'll be asked for an emailed code next time you sign in.".to_string()
            } else {
                "Your account no longer asks for a code at sign-in.".to_string()
            },
            image_id: None,
            related_id: None,
            link: Some("/account/profile".to_string()),
            email: None,
        },
    ).await;

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "two_factor_enabled": body.enabled,
    })))
}
