/* DEFINE ALL OF YOUR CRON ROUTINES HERE */

/// Deletes refresh-token rows whose tokens have expired.
///
/// One row per session means the table grows with every sign-in rather than
/// staying at one row per user, so something has to reclaim them. `new()` prunes
/// the signing-in account's own rows; this catches accounts that stopped signing
/// in altogether.
pub async fn purge_expired_sessions() {
    match crate::BuiltIns::jwt::refresh_token::purge_expired() {
        Ok(0) => {}
        Ok(count) => log::info!("cron: purged {} expired session(s)", count),
        Err(error) => log::error!("cron: could not purge expired sessions: {:?}", error),
    }
}
