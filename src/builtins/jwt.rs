/* JSON Web Tokens Implementation */

use std::env;
use chrono::Utc;
use rusqlite::params;
use crate::BuiltIns::sqlite::{ self, Result as SqliteResult };
use serde::{ Serialize, Deserialize };
use crate::Model::Account::AccountRole;
use jsonwebtoken::{ encode, decode, Header, Validation, EncodingKey, DecodingKey };

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

/// Errors from minting a token. Both variants used to be panics: a missing key
/// was `.expect()` and a failed `encode()` was `.unwrap()`, so a config mistake
/// took the worker down mid-request instead of returning 500.
#[derive(Debug)]
pub enum Error {
  /// A required signing key is missing from the environment.
  Config(String),
  /// `jsonwebtoken` failed to sign the claims.
  Encode(jsonwebtoken::errors::Error),
}

impl std::fmt::Display for Error {
  fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Error::Config(message) => write!(fmt, "{}", message),
      Error::Encode(error) => write!(fmt, "jwt encode: {}", error),
    }
  }
}

impl std::error::Error for Error {}

/// Reads a signing key, or reports which variable is missing.
fn secret(name: &str) -> std::result::Result<String, Error> {
  env::var(name)
    .ok()
    .filter(|value| !value.is_empty())
    .ok_or_else(|| Error::Config(format!("{} must be set on .env file", name)))
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

  /// Mints a 15-minute access token. Returns the token and its lifetime in minutes.
  pub fn generate_default(
    user_id: &str,
    role: AccountRole,
  ) -> std::result::Result<(String, u64), Error> {
    const LIFETIME_MINUTES: u64 = 15;

    let access_token_details = Claims {
      sub: user_id.to_string(),
      role,
      iss: crate::Constant::project_name().to_string(),
      iat: current_time(),
      exp: expire_at(Time::Minute(LIFETIME_MINUTES))
    };

    Ok((create(access_token_details)?, LIFETIME_MINUTES))
  }

  #[allow(dead_code)]
  pub fn create(claims: Claims) -> std::result::Result<String, Error> {
    let key = secret("JWT_LOCAL_ACCESS_KEY")?;
    let secret_key = EncodingKey::from_secret(key.as_ref());

    encode(&Header::default(), &claims, &secret_key).map_err(Error::Encode)
  }

  /// Verifies an access token's signature, expiry and issuer.
  ///
  /// `Validation::default()` checks the signature and `exp` but NOT `iss`, so
  /// until now the `iss` claim was written on every token and read by nobody —
  /// a token minted by any other service sharing the secret would have passed.
  /// The expected issuer is pinned per key:
  ///   - `Local`  — our own tokens, so it must equal `Constant::project_name()`
  ///   - `Remote` — a partner's tokens, so it must equal `JWT_REMOTE_ISSUER`.
  ///     Unset means we cannot say who we trust, so verification fails closed
  ///     rather than accepting any issuer.
  #[allow(dead_code)]
  pub fn verify(token: &str, key: Key) -> Result<Claims, String> {
    let (key, issuer) = match key {
      Key::Local => (
        secret("JWT_LOCAL_ACCESS_KEY").map_err(|error| error.to_string())?,
        crate::Constant::project_name().to_string(),
      ),
      Key::Remote => (
        secret("JWT_REMOTE_KEY").map_err(|error| error.to_string())?,
        secret("JWT_REMOTE_ISSUER").map_err(|error| error.to_string())?,
      ),
    };

    let mut validation = Validation::default();
    validation.set_issuer(&[issuer]);

    let token_data = decode::<Claims>(
      token,
      &DecodingKey::from_secret(key.as_ref()),
      &validation
    );

    match token_data {
      Ok(token_data) => Ok(token_data.claims),
      Err(error) => Err(format!("{:?}", error.kind()))
    }
  }
}

pub mod refresh_token {
  use super::*;

  /// `jti` is a per-mint unique id, and it is load-bearing for rotation.
  ///
  /// `iat`/`exp` have one-second resolution, so two rotations inside the same
  /// second produced byte-identical claims and therefore an identical token
  /// string - the "new" token was the old one, and single-use silently stopped
  /// holding. A random id per mint makes every token distinct.
  #[derive(Debug, Serialize, Deserialize)]
  struct Claims { sub: String, jti: String, iat: u64, exp: u64 }

  /// Refresh-token lifetime. Idle clients must re-authenticate after this.
  const REFRESH_TOKEN_DAYS: u64 = 90;

  /* Decode a refresh token and return its subject (user_id). */
  #[allow(dead_code)]
  pub fn subject(token: &str) -> Result<String, String> {
    let key = super::secret("JWT_LOCAL_REFRESH_KEY")
      .map_err(|error| error.to_string())?;

    /*
      `Validation::default()` requires `exp` and rejects an expired token.

      The previous code cleared `required_spec_claims`, and the Claims struct had
      no `exp` field to begin with - so refresh tokens carried no expiry and none
      was demanded. The comment here claimed otherwise. Both halves are fixed
      now: tokens are minted with `exp`, and this validates it.
    */
    let validation = Validation::default();

    match decode::<Claims>(
      token,
      &DecodingKey::from_secret(key.as_ref()),
      &validation,
    ) {
      Ok(data) => Ok(data.claims.sub),
      Err(error) => Err(format!("{:?}", error.kind())),
    }
  }

  #[allow(dead_code)]
  fn gen_token(issuer: &str) -> std::result::Result<String, super::Error> {
    let key = super::secret("JWT_LOCAL_REFRESH_KEY")?;
    let secret_key = EncodingKey::from_secret(key.as_ref());

    encode(
      &Header::default(),
      &Claims {
        sub: issuer.to_owned(),
        jti: uuid::Uuid::new_v4().to_string(),
        iat: super::current_time(),
        exp: super::expire_at(Time::Day(REFRESH_TOKEN_DAYS)),
      },
      &secret_key
    ).map_err(super::Error::Encode)
  }

  /// Opens a NEW session for `issuer` and returns its refresh token.
  ///
  /// Every call creates a row. It used to look for an existing row and hand the
  /// same token back, because the table allowed only one per user - so signing
  /// in on a second device silently invalidated the first. One row per session
  /// is what makes multiple devices work.
  ///
  /// Expired rows for this account are pruned first, so the table does not grow
  /// without bound as someone signs in over the years. Only this account's rows
  /// are touched, keeping the work proportional to one user rather than the
  /// whole table.
  #[allow(dead_code)]
  pub fn new(issuer: &str) -> Result<String, String> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)
      .map_err(|error| { log::error!("{:?}", error); error.to_string() })?;

    let cutoff = Utc::now().timestamp_millis()
      - (REFRESH_TOKEN_DAYS as i64) * 24 * 60 * 60 * 1000;

    if let Err(error) = db_conn.execute(
      "DELETE FROM refreshToken WHERE issuer = ?1 AND created_at < ?2",
      params![issuer, cutoff]
    ) {
      // Not fatal: a stale row is clutter, not a correctness problem, and the
      // caller is in the middle of signing someone in.
      log::warn!("could not prune expired sessions for {}: {:?}", issuer, error);
    }

    let token = gen_token(issuer)
      .map_err(|error| { log::error!("{:?}", error); error.to_string() })?;

    let result = db_conn.execute("
      INSERT INTO refreshToken (token, issuer, status, created_at)
      VALUES (?1, ?2, ?3, ?4)",
      params![&token, issuer, Status::Active.to_string(), Utc::now().timestamp_millis()]
    );

    match result {
      Ok(1) => Ok(token),
      Ok(count) => {
        let error = format!("Result value should be 1. Found: {}", count);
        log::error!("{}", error);
        Err(error)
      }
      Err(error) => {
        log::error!("{:?}", error);
        Err(error.to_string())
      }
    }
  }

  /// Every live session for an account, newest first.
  ///
  /// This is the point of keying on `token`: a "signed-in devices" screen is now
  /// a query rather than an impossibility.
  #[allow(dead_code)]
  pub fn sessions(issuer: &str) -> SqliteResult<Vec<TokenDetails>> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)?;

    let mut stmt = db_conn.prepare_cached(
      "SELECT token, issuer, status, created_at, modified_at
       FROM refreshToken WHERE issuer = ?1 ORDER BY created_at DESC"
    )?;

    let rows = stmt.query_map(params![issuer], |row| {
      Ok(TokenDetails {
        token: row.get(0)?,
        issuer: row.get(1)?,
        status: row.get(2)?,
        created_at: row.get(3)?,
        modified_at: row.get(4)?,
      })
    })?;

    // Map rusqlite's error into the module's own error type, which is what the
    // rest of this file returns.
    let mut sessions = Vec::new();
    for row in rows {
      sessions.push(row?);
    }

    Ok(sessions)
  }

  /// Revokes every session for an account - "sign out everywhere".
  ///
  /// Returns how many were revoked. Impossible to express before this change,
  /// since there was only ever one row to block.
  #[allow(dead_code)]
  pub fn block_all(issuer: &str) -> SqliteResult<usize> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)?;

    Ok(db_conn.execute("
      UPDATE refreshToken SET status = ?1, modified_at = ?2
      WHERE issuer = ?3 AND status = ?4",
      params![
        Status::Blocked.to_string(),
        Utc::now().timestamp_millis(),
        issuer,
        Status::Active.to_string()
      ]
    )?)
  }

  /// Removes every session row for an account outright.
  ///
  /// Distinct from `block_all`, which marks rows Blocked and leaves them for the
  /// audit trail. Account deletion needs them gone: `issuer` IS the user id, so
  /// rows surviving a "delete my account" request would leave that id on disk in
  /// a table the user asked to be removed from.
  #[allow(dead_code)]
  pub fn delete_all(issuer: &str) -> SqliteResult<usize> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)?;

    Ok(db_conn.execute(
      "DELETE FROM refreshToken WHERE issuer = ?1",
      params![issuer]
    )?)
  }

  /// Deletes expired rows for every account. For the cleanup cron.
  #[allow(dead_code)]
  pub fn purge_expired() -> SqliteResult<usize> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)?;

    let cutoff = Utc::now().timestamp_millis()
      - (REFRESH_TOKEN_DAYS as i64) * 24 * 60 * 60 * 1000;

    Ok(db_conn.execute(
      "DELETE FROM refreshToken WHERE created_at < ?1",
      params![cutoff]
    )?)
  }

  #[allow(dead_code)]
  pub fn status(token: &str) -> SqliteResult<Option<Status>> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)?;

    let mut stmt = db_conn.prepare_cached(
      "SELECT status FROM refreshToken WHERE token = ?1"
    )?;

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
      Err(error) => Err(error.into())
    }
  }

  /// Exchanges a valid refresh token for a fresh one, invalidating the old.
  ///
  /// Rotation is what makes a stolen refresh token a bounded problem: the token
  /// is single-use, so the copy the attacker holds stops working as soon as the
  /// legitimate client refreshes (and vice versa, which surfaces the theft).
  /// Without it the same string stayed valid for the life of the account.
  ///
  /// Verifies before touching the database, in this order: signature and expiry,
  /// then that the token is the Active one on record, then that the row belongs
  /// to the subject the token names.
  ///
  /// Rotates ONE session. The row is keyed on `token`, so the other devices
  /// signed in to the same account keep their own tokens and stay signed in.
  /// Returns `(issuer, new_token)`. The issuer is the identity the presented
  /// token actually proved - callers must use it rather than trusting whatever
  /// the request body claims.
  #[allow(dead_code)]
  pub fn rotate(token: &str) -> Result<(String, String), String> {
    let subject = subject(token)?;

    let db_conn = sqlite::connect(sqlite::DBF::JWT)
      .map_err(|error| { log::error!("{:?}", error); error.to_string() })?;

    let mut stmt = db_conn.prepare_cached(
      "SELECT issuer, status FROM refreshToken WHERE token = ?1"
    ).map_err(|error| { log::error!("{:?}", error); error.to_string() })?;

    let row = stmt.query_row(params![token], |row| {
      Ok((row.get::<usize, String>(0)?, row.get::<usize, String>(1)?))
    });

    let (issuer, status) = match row {
      Ok(row) => row,
      // No such token on record - already rotated away, or never issued here.
      Err(_) => return Err("refresh token is not recognised".to_string()),
    };

    if status != Status::Active.to_string() {
      return Err("refresh token has been revoked".to_string());
    }

    // A correctly signed token whose subject does not match the row it was found
    // in means the store and the token disagree; refuse rather than guess.
    if issuer != subject {
      log::error!("refresh token subject {} does not match issuer {}", subject, issuer);
      return Err("refresh token does not match its account".to_string());
    }

    /*
      Swap the token on THIS row, so the session survives and only its token
      changes. Scoped by the presented token rather than by issuer - keyed by
      issuer it would have rewritten whichever single row the account had, which
      is precisely how rotating on one device used to sign out the others.

      `WHERE token = ?` also makes this safely single-use under a race: two
      concurrent refreshes with the same old token both compute a new one, but
      only the first UPDATE matches a row. The loser sees 0 rows changed and is
      told the token is not recognised, which is the correct answer - it has
      already been rotated away.
    */
    let new_token = gen_token(&issuer)
      .map_err(|error| { log::error!("{:?}", error); error.to_string() })?;

    let changed = db_conn.execute("
      UPDATE refreshToken SET token = ?1, modified_at = ?2
      WHERE token = ?3 AND status = ?4",
      params![
        &new_token,
        Utc::now().timestamp_millis(),
        token,
        Status::Active.to_string()
      ]
    ).map_err(|error| { log::error!("{:?}", error); error.to_string() })?;

    if changed != 1 {
      return Err("refresh token is not recognised".to_string());
    }

    Ok((issuer, new_token))
  }

  #[allow(dead_code)]
  pub fn block(token: &str) -> SqliteResult<bool> {
    let db_conn = sqlite::connect(sqlite::DBF::JWT)?;

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
