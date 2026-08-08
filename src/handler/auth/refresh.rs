use crate::utils::response::Response;
use crate::{BuiltIns::jwt, Model::Account::AccountRole};
use actix_web::{Error, HttpResponse, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqBody {
    refresh_token: String,
    user_id: String,
    role: AccountRole,
}

pub async fn task(form_data: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let refresh_token = form_data.refresh_token.to_string();

    let result = jwt::refresh_token::status(&refresh_token);

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::unauthorized(&error.to_string()));
    }

    let option = result.unwrap();
    if let None = option {
        return Ok(Response::forbidden("status not found on this token"));
    }

    let status = option.unwrap();

    match status {
        jwt::Status::Active => {
            let (access_token, time_in_minutes) =
                jwt::access_token::generate_default(&form_data.user_id, form_data.role.clone());

            let access_token_valid_till =
                Utc::now().timestamp_millis() + (time_in_minutes * 60 * 1000) as i64;

            return Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "access_token": access_token,
                    "access_token_valid_till": access_token_valid_till
                })));
        }
        jwt::Status::Blocked => {
            return Ok(Response::forbidden("this token is blocked"));
        }
    }
}
