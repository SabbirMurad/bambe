/* JSON Web Tokens Implementation */

use std::env;
use chrono::Utc;
use crate::BuiltIns;
use rusqlite::{ Error, params };
use serde::{ Serialize, Deserialize };
use crate::Model::Account::AccountRole;
use jsonwebtoken::{ encode, decode, Header, Validation, EncodingKey, DecodingKey };

const PROJECT_NAME: &str = "Hasp";

/*
  To see general Registered Claims visit
  https://auth0.com/docs/secure/tokens/json-web-tokens/json-web-token-claims#registered-claims

  To see all available Registered Claims visit
  https://www.iana.org/assignments/jwt/jwt.xhtml#claims

  Add / Remove Claim fields based on you project needs!
*/

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  pub sub: String,
  pub role: AccountRole,
  pub iss: String,
  pub iat: u64,
  pub exp: u64
}

/* For Loading Dynamic Key */
#[allow(dead_code)]
pub enum Key { Local, Remote }

/* For Setting Expiration Time */
#[allow(dead_code)]
pub enum Time { Minute(u64), Hour(u64), Day(u64) }

/* For Setting Token Status */
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub enum Status { Active, Blocked }

impl std::fmt::Display for Status {
  fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Status::Active => write!(fmt, "Active"),
      Status::Blocked => write!(fmt, "Blocked"),
    }
  }
}

/* For Refresh Token Details */
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenDetails {
  pub issuer: String,
  pub token: String,
  pub status: String,
  pub created_at: usize,
  pub modified_at: Option<usize>
}


pub mod access_token {
  use super::*;

  pub fn generate_default(user_id: &str, role: AccountRole) -> (String, u64) {
    //sending the auth token and refresh token
    let access_token_details = Claims {
      sub: user_id.to_string(),
      role,
      iss: PROJECT_NAME.to_string(),
      iat: current_time(),
      exp: expire_at(Time::Minute(15))
    };
  
    let auth_token = create(access_token_details);
  
    (auth_token, 15)
  }

  #[allow(dead_code)]
  pub fn create(claims: Claims) -> String {
    let key = env::var("JWT_LOCAL_ACCESS_KEY")
    .expect("JWT_LOCAL_ACCESS_KEY must be set on .env file");

    let secret_key = EncodingKey::from_secret(key.as_ref());
    let token = encode(&Header::default(), &claims, &secret_key).unwrap();

    token
  }

  #[allow(dead_code)]
  pub fn verify(token: &str, key: Key) -> Result<Claims, String> {
    let key = match key {
      Key::Local => {
        env::var("JWT_LOCAL_ACCESS_KEY")
        .expect("JWT_LOCAL_ACCESS_KEY must be set on .env file")
      }
      Key::Remote => {
        env::var("JWT_REMOTE_KEY")
        .expect("JWT_REMOTE_KEY must be set on .env file")
      }
    };

    let token_data = decode::<Claims>(
      token,
      &DecodingKey::from_secret(key.as_ref()),
      &Validation::default()
    );

    match token_data {
      Ok(token_data) => Ok(token_data.claims),
      Err(error) => Err(format!("{:?}", error.kind()))
    }
  }
}

pub mod refresh_token {
  use super::*;

  #[derive(Debug, Serialize, Deserialize)]
  struct Claims { sub: String, iat: u64 }

  #[allow(dead_code)]
  fn gen_token(issuer: &str) -> String {
    let key = env::var("JWT_LOCAL_REFRESH_KEY")
    .expect("JWT_LOCAL_REFRESH_KEY must be set on .env file");

    let secret_key = EncodingKey::from_secret(key.as_ref());
    let token = encode(
      &Header::default(),
      &Claims { sub: issuer.to_owned(), iat: super::current_time() },
      &secret_key
    ).unwrap();

    token
  }

  #[allow(dead_code)]
  pub fn new(issuer: &str) -> Result<String, String> {
    let db_conn = BuiltIns::sqlite::connect(
      BuiltIns::sqlite::DBF::JWT
    ).unwrap();

    let mut stmt = db_conn.prepare_cached(
      "SELECT * FROM refreshToken WHERE issuer = ?1"
    ).unwrap();

    let result = stmt.exists(params![issuer]);
    if let Err(error) = result {
      log::error!("{:?}", error);
      return Err(error.to_string());
    }

    let exist = result.unwrap();
    if exist {
      let mut stmt = db_conn.prepare_cached(
        "SELECT * FROM refreshToken WHERE issuer = ?1"
      ).unwrap();

      match stmt.query_row(params![issuer], |row| {
        Ok(TokenDetails {
          issuer: row.get(0)?,
          token: row.get(1)?,
          status: row.get(2)?,
          created_at: row.get(3)?,
          modified_at: row.get(4)?,
        })
      }) {
        Ok(result) => {
          if result.status == "Blocked" {
            renew(issuer)
          }
          else {
            Ok(result.token)
          }
        },
        Err(error) => {
          log::error!("{:?}", error);
          Err(error.to_string())
        }
      }
    } else {
      let token = gen_token(issuer);
      let time_stamp = Utc::now().timestamp_millis();

      let result = db_conn.execute("
        INSERT INTO refreshToken (issuer, token, status, created_at)
        VALUES (?1, ?2, ?3, ?4)",
        (&issuer, &token, Status::Active.to_string(), time_stamp)
      );

      if let Err(error) = result {
        log::error!("{:?}", error);
        return Err(error.to_string());
      }

      let count = result.unwrap();
      if count == 1 {
        Ok(token)
      }
      else {
        let error = format!("Result value should be 1. Found: {}", count);
        log::error!("{}", error);
        Err(error)
      }
    }
  }

  #[allow(dead_code)]
  pub fn renew(issuer: &str) -> Result<String, String> {
    let db_conn = BuiltIns::sqlite::connect(
      BuiltIns::sqlite::DBF::JWT
    ).unwrap();

    let token = gen_token(issuer);
    let time_stamp = Utc::now().timestamp_millis();

    let result = db_conn.execute("
      UPDATE refreshToken SET token = ?1, modified_at = ?2
      WHERE issuer = ?3",
      params![&token, time_stamp, issuer]
    );

    if let Err(error) = result {
      log::error!("{:?}", error);
      return Err(error.to_string());
    }

    let count = result.unwrap();
    if count == 1 {
      Ok(token)
    }
    else {
      let error = format!("Result value should be 1. Found: {}", count);
      log::error!("{}", error);
      Err(error)
    }
  }
  #[allow(dead_code)]
  pub fn details(issuer: &str) -> Result<TokenDetails, Error> {
    let db_conn = BuiltIns::sqlite::connect(
      BuiltIns::sqlite::DBF::JWT
    ).unwrap();

    let mut stmt = db_conn.prepare_cached(
      "SELECT * FROM refreshToken WHERE issuer = ?1"
    ).unwrap();

    match stmt.query_row(
      params![issuer], |row| {
        Ok(TokenDetails {
          issuer: row.get(0)?,
          token: row.get(1)?,
          status: row.get(2)?,
          created_at: row.get(3)?,
          modified_at: row.get(4)?,
        })
      }
    ) {
      Ok(token_details) => Ok(token_details),
      Err(error) => Err(error)
    }
  }

  #[allow(dead_code)]
  pub fn status(token: &str) -> Result<Option<Status>, Error> {
    let db_conn = BuiltIns::sqlite::connect(
      BuiltIns::sqlite::DBF::JWT
    ).unwrap();

    let mut stmt = db_conn.prepare_cached(
      "SELECT status FROM refreshToken WHERE token = ?1"
    ).unwrap();

    struct Data { status: String }

    match stmt.query_row(
      params![token], |row| {
        Ok(Data { status:  row.get(0)? })
      }
    ) {
      Ok(data) => {
        if data.status == "Active" { Ok(Some(Status::Active)) }
        else if data.status == "Blocked" { Ok(Some(Status::Blocked)) }
        else { Ok(None) } 
      },
      Err(error) => Err(error)
    }
  }

  #[allow(dead_code)]
  pub fn block(token: &str) -> Result<bool, Error> {
    let db_conn = BuiltIns::sqlite::connect(
      BuiltIns::sqlite::DBF::JWT
    ).unwrap();

    let result = db_conn.execute("
      UPDATE refreshToken SET status = ?1
      WHERE token = ?2",
      params![Status::Blocked.to_string(), &token]
    )?;

    if result == 1 { Ok(true) }
    else {
      log::error!("Result: {} | Perhaps token doesn't exists!", result);
      Ok(false)
    }
  }
}

/* Returns EPOCH time in seconds */
pub fn current_time() -> u64 { jsonwebtoken::get_current_timestamp() }

/* Returns EPOCH time in seconds */
#[allow(dead_code)]
pub fn expire_at(time: Time) -> u64 {
  let mut time_stamp = current_time();

  match time {
    Time::Minute(val) => time_stamp += val * 60,
    Time::Hour(val) => time_stamp += val * 60 * 60,
    Time::Day(val) => time_stamp += val * 24 * 60 * 60
  }

  time_stamp
}
