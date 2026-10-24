use mongodb::bson::doc;
use crate::Model::Account;
use actix_web::HttpResponse;
use mongodb::{ClientSession, Database, IndexModel};
use mongodb::options::IndexOptions;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;

pub mod refresh;
pub use refresh as Refresh;

pub mod sign_up;
pub use sign_up as SignUp;

pub mod admin_sign_up;
pub use admin_sign_up as AdminSignUp;

pub mod admin_sign_in;
pub use admin_sign_in as AdminSignIn;

pub mod verify_email;
pub use verify_email as VerifyEmail;

pub mod resend_verification_code;
pub use resend_verification_code as ResendVerificationCode;

pub mod sign_in;
pub use sign_in as SignIn;

pub mod sign_out;
pub use sign_out as SignOut;

pub mod forgot_password;
pub use forgot_password as ForgotPassword;

pub mod verify_sign_in_code;
pub use verify_sign_in_code as VerifySignInCode;

pub mod verify_reset_code;
pub use verify_reset_code as VerifyResetCode;

pub mod reset_password;
pub use reset_password as ResetPassword;

pub mod change_password;
pub use change_password as ChangePassword;


pub mod social_login;
pub use social_login as SocialLogin;


/// Unique email indexes for the two account collections. Users and admins are
/// separate collections, so the same email can exist once in each — but at most
/// once per collection. Idempotent; safe to call every startup.
pub async fn ensure_indexes() {
    let db = MongoDB.connect();

    let user_index = IndexModel::builder()
        .keys(doc!{ "email_address": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    if let Err(error) = db
        .collection::<Account::AccountCore>("account_core")
        .create_index(user_index)
        .await
    {
        log::warn!("account_core email index: {:?}", error);
    }

    let admin_index = IndexModel::builder()
        .keys(doc!{ "email_address": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    if let Err(error) = db
        .collection::<Account::AdminAccount>("admin_account")
        .create_index(admin_index)
        .await
    {
        log::warn!("admin_account email index: {:?}", error);
    }

    // One account per Google/Facebook identity; see social_login.rs.
    let identity_index = IndexModel::builder()
        .keys(doc!{ "firebase_uid": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    if let Err(error) = db
        .collection::<mongodb::bson::Document>("social_identity")
        .create_index(identity_index)
        .await
    {
        log::warn!("social_identity index: {:?}", error);
    }

}

/*
  Replaces every plaintext password left by the old storage scheme with an
  Argon2 hash.

  It can do this without anyone signing in, because a plaintext row *is* the
  password — so it can be hashed directly. That matters: the fallback in
  `util::password::verify` would otherwise keep reading plaintext for as long
  as an account went untouched, which for an abandoned account is forever.

  Idempotent. A row already stored as a PHC string is skipped, so this is safe
  to run on every startup and safe to interrupt — anything it has not reached
  yet still authenticates through the legacy path.
*/
pub async fn migrate_plaintext_passwords() {
    use crate::util::password;

    let db = MongoDB.connect();
    let mut migrated = 0_u64;
    let mut failed = 0_u64;

    // account_core and admin_account hold passwords in the same shape, but
    // they are different types, so each is walked on its own.
    let users = db.collection::<Account::AccountCore>("account_core");
    match users.find(doc!{}).await {
        Ok(mut cursor) => {
            use futures::StreamExt;
            while let Some(row) = cursor.next().await {
                let Ok(account) = row else { continue };
                // An empty password is a social-login account with none to migrate.
                if account.password.is_empty() || password::is_hashed(&account.password) {
                    continue;
                }
                match password::hash(&account.password) {
                    Ok(hashed) => {
                        if let Err(error) = users.update_one(
                            doc!{ "uuid": &account.uuid },
                            doc!{ "$set": { "password": &hashed } },
                        ).await {
                            log::error!("password migration (user {}): {:?}", account.uuid, error);
                            failed += 1;
                        } else {
                            migrated += 1;
                        }
                    }
                    Err(error) => {
                        log::error!("password migration (user {}): {error}", account.uuid);
                        failed += 1;
                    }
                }
            }
        }
        Err(error) => log::error!("password migration: could not read account_core: {:?}", error),
    }

    let admins = db.collection::<Account::AdminAccount>("admin_account");
    match admins.find(doc!{}).await {
        Ok(mut cursor) => {
            use futures::StreamExt;
            while let Some(row) = cursor.next().await {
                let Ok(admin) = row else { continue };
                if admin.password.is_empty() || password::is_hashed(&admin.password) {
                    continue;
                }
                match password::hash(&admin.password) {
                    Ok(hashed) => {
                        if let Err(error) = admins.update_one(
                            doc!{ "uuid": &admin.uuid },
                            doc!{ "$set": { "password": &hashed } },
                        ).await {
                            log::error!("password migration (admin {}): {:?}", admin.uuid, error);
                            failed += 1;
                        } else {
                            migrated += 1;
                        }
                    }
                    Err(error) => {
                        log::error!("password migration (admin {}): {error}", admin.uuid);
                        failed += 1;
                    }
                }
            }
        }
        Err(error) => log::error!("password migration: could not read admin_account: {:?}", error),
    }

    if migrated > 0 || failed > 0 {
        log::info!("Password migration: {migrated} hashed, {failed} failed");
    }
}


pub async fn delete_account(
    db: &Database,
    session: &mut ClientSession,
    user_id: &str
) -> Result<(), HttpResponse> {
    let collection = db.collection::<Account::AccountCore>("account_core");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(session).await;
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let collection = db.collection::
    <Account::AccountProfile>("account_profile");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(session).await;
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let collection = db.collection::
    <Account::AccountVerificationRequest>("account_verification_request");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        MongoDB::abort(session).await;
        return Err(Response::internal_server_error(&error.to_string()));
    }

    Ok(())
}

/// Wrong guesses a 6-digit code may take before it is destroyed. A code with no
/// limit is brute-forceable inside its 15-minute life at the rate limiter's pace.
pub const MAX_CODE_ATTEMPTS: u32 = 5;

/// Records a wrong guess against a verification request and reports whether it
/// has now used up its guesses (in which case it has been deleted, and the user
/// must ask for a new code).
///
/// Deliberately NOT part of the caller's transaction: the wrong-guess path
/// aborts, and a counter incremented inside the transaction would be rolled
/// back with it - so the limit would never fire. This write has to survive the
/// rejection it is recording.
pub async fn record_wrong_code(db: &Database, collection: &str, request_uuid: &str) -> bool {
    let col = db.collection::<mongodb::bson::Document>(collection);
    let updated = col
        .find_one_and_update(doc! { "uuid": request_uuid }, doc! { "$inc": { "attempts": 1 } })
        .return_document(mongodb::options::ReturnDocument::After)
        .await
        .ok()
        .flatten();

    let attempts = updated
        .as_ref()
        .and_then(|d| d.get_i32("attempts").ok().map(|n| n as i64).or_else(|| d.get_i64("attempts").ok()))
        .unwrap_or(0);

    if attempts >= MAX_CODE_ATTEMPTS as i64 {
        col.delete_one(doc! { "uuid": request_uuid }).await.ok();
        return true;
    }
    false
}
