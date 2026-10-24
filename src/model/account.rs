use serde::{Deserialize, Serialize};

//role for account
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountRole { Administrator, User}
impl std::fmt::Display for AccountRole {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt,"{:?}", self)
    }
}

//account_core
#[derive(Debug, Deserialize, Serialize)]
pub struct AccountCore {
    pub uuid: String,
    pub email_address: String,
    pub username: String,
    pub password: String,
    pub email_verified: bool,
    pub role: AccountRole,
    pub two_a_factor_auth_enabled: bool,
    pub two_a_factor_auth_updated: Option<i64>,

    pub created_at: i64,
    pub suspended_at: Option<i64>,
    pub suspended_by: Option<String>,
}

// admin_account — administrators live in their own collection, separate from
// shoppers. Minimal: just what's needed to sign in to the dashboard. This lets
// the same email exist as both a customer and an admin.
#[derive(Debug, Deserialize, Serialize)]
pub struct AdminAccount {
    pub uuid: String,
    pub email_address: String,
    pub name: String,
    pub password: String,
    pub created_at: i64,
}

/*
  account_profile — who the person is.

  What they buy lives in customer_profile, not here: this row is written once
  at sign-up, while the commerce rollups are rewritten on every order.
*/
#[derive(Debug, Deserialize, Serialize)]
pub struct AccountProfile {
    pub uuid: String,

    pub first_name: String,
    pub last_name: String,
    pub phone_number: Option<String>,
    /// The picture the customer uploaded here. Theirs to set, and nothing
    /// a sign-in does ever changes it.
    pub profile_picture: Option<String>,
    /// A copy of the photo from their latest Google or Facebook sign-in,
    /// refreshed on each one. Shown only while `profile_picture` is empty,
    /// so a provider photo is a default, never an override.
    #[serde(default)]
    pub social_picture: Option<String>,

    pub modified_at: i64,
}

//account_verification_request
#[derive(Debug, Deserialize, Serialize)]
pub struct AccountVerificationRequest {
    pub uuid: String,
    pub user_id: String,
    pub code: String,
    pub expires_at: i64,
    // Wrong guesses so far; the request is destroyed at MAX_CODE_ATTEMPTS.
    #[serde(default)]
    pub attempts: u32,
}

//account_deletion_request — a user-initiated request to delete their account.
//The account is NOT removed automatically; it sits in the admin dashboard
//queue until an admin manually deletes it.
#[derive(Debug, Deserialize, Serialize)]
pub struct AccountDeletionRequest {
    pub uuid: String,
    pub user_id: String,
    pub email: String,
    pub reason: String,
    pub details: Option<String>,
    pub requested_at: i64,
}

//password_reset_request
#[derive(Debug, Deserialize, Serialize)]
pub struct PasswordResetRequest {
    pub uuid: String,
    pub user_id: String,
    pub secret_key: String,
    pub validation_code: String,
    pub code_validated: bool,
    pub expires_at: i64,
    // Wrong guesses so far; the request is destroyed at MAX_CODE_ATTEMPTS.
    #[serde(default)]
    pub attempts: u32,
}

//sign_in_verification_request
#[derive(Debug, Deserialize, Serialize)]
pub struct SignInVerificationRequest {
    pub uuid: String,
    pub user_id: String,
    pub validation_code: String,
    pub expires_at: i64,
    // Wrong guesses so far; the request is destroyed at MAX_CODE_ATTEMPTS.
    #[serde(default)]
    pub attempts: u32,
}

//fcm_token
#[derive(Debug, Deserialize, Serialize)]
pub struct FcmToken {
    pub uuid: String,
    pub token: String,
    pub created_at: i64,
}
