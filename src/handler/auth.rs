use mongodb::bson::doc;
use crate::Model::Account;
use actix_web::HttpResponse;
use mongodb::{ClientSession, Database, IndexModel};
use mongodb::options::IndexOptions;
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;

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

pub mod get;
pub use get as Get;

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

}

/*
  Replaces every plaintext password left by the old storage scheme with an
  Argon2 hash.

  It can do this without anyone signing in, because a plaintext row *is* the
  password — so it can be hashed directly. That matters: the fallback in
  `utils::password::verify` would otherwise keep reading plaintext for as long
  as an account went untouched, which for an abandoned account is forever.

  Idempotent. A row already stored as a PHC string is skipped, so this is safe
  to run on every startup and safe to interrupt — anything it has not reached
  yet still authenticates through the legacy path.
*/
pub async fn migrate_plaintext_passwords() {
    use crate::utils::password;

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
        session.abort_transaction().await.ok().unwrap();
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let collection = db.collection::
    <Account::AccountProfile>("account_profile");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Err(Response::internal_server_error(&error.to_string()));
    }

    let collection = db.collection::
    <Account::AccountVerificationRequest>("account_verification_request");
    let result = collection.delete_one(
        doc!{"uuid": user_id},
    ).session(&mut *session).await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        session.abort_transaction().await.ok().unwrap();
        return Err(Response::internal_server_error(&error.to_string()));
    }

    Ok(())
}