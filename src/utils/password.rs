/*
  Password storage.

  Every password on this server is stored as an Argon2id PHC string —
  `$argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>` — which carries its
  own random salt and its own cost parameters. Raising those parameters
  later therefore does not strand the hashes already written.

  Nothing outside this module should touch a stored password. Handlers
  call `hash` on the way in and `verify` on the way out, and never
  compare two password strings themselves.

  ── On the rows written before this module existed ──

  This server previously stored passwords as plaintext. `verify` still
  accepts a stored value that is not a PHC string, comparing it
  literally, so an account written under the old scheme can still sign
  in. That path exists only so the migration in `migrate_plaintext`
  cannot lock anyone out if it is interrupted; it reports the fallback
  through `Verified::NeedsRehash` so the caller can upgrade the row on
  the spot, and it is expected to stop firing once the migration has
  run.
*/

use argon2::Argon2;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};

/// What `verify` concluded about a stored password.
#[derive(Debug, PartialEq, Eq)]
pub enum Verified {
    /// Correct, and stored in the current format. Nothing to do.
    Ok,
    /// Correct, but the stored value was plaintext or an older format.
    /// The caller should write `hash(password)` back to the row.
    NeedsRehash,
    /// Wrong password, or a row with no usable password at all.
    Failed,
}

/// True when `stored` is a PHC string rather than a legacy plaintext value.
pub fn is_hashed(stored: &str) -> bool {
    stored.starts_with("$argon2")
}

/// Hashes a password for storage. The salt is generated per call, so two
/// accounts with the same password never share a hash.
pub fn hash(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("could not hash password: {e}"))
}

/// Checks `attempt` against a stored password.
pub fn verify(attempt: &str, stored: &str) -> Verified {
    /*
      An empty stored password means the account has no password to
      check against — social_login.rs creates accounts that way. Such an
      account must never authenticate by password, and in particular an
      empty attempt must not match an empty stored value.
    */
    if stored.is_empty() {
        return Verified::Failed;
    }

    if is_hashed(stored) {
        let parsed = match PasswordHash::new(stored) {
            Ok(p) => p,
            // A malformed hash is a corrupt row, not a valid password.
            Err(_) => return Verified::Failed,
        };
        return match Argon2::default().verify_password(attempt.as_bytes(), &parsed) {
            Ok(()) => Verified::Ok,
            Err(_) => Verified::Failed,
        };
    }

    // Legacy plaintext row — see the note at the top of this module.
    if attempt == stored {
        Verified::NeedsRehash
    } else {
        Verified::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_salted_per_call() {
        let a = hash("correct horse").unwrap();
        let b = hash("correct horse").unwrap();
        assert_ne!(a, b, "same password must not produce the same hash twice");
        assert_eq!(verify("correct horse", &a), Verified::Ok);
        assert_eq!(verify("correct horse", &b), Verified::Ok);
    }

    #[test]
    fn wrong_password_fails() {
        let stored = hash("correct horse").unwrap();
        assert_eq!(verify("Correct horse", &stored), Verified::Failed);
        assert_eq!(verify("", &stored), Verified::Failed);
    }

    #[test]
    fn legacy_plaintext_verifies_but_asks_for_a_rehash() {
        assert_eq!(verify("hunter2", "hunter2"), Verified::NeedsRehash);
        assert_eq!(verify("hunter3", "hunter2"), Verified::Failed);
    }

    /* social_login.rs writes an empty password. Nothing may sign in as
       that account by password, least of all by sending nothing. */
    #[test]
    fn passwordless_accounts_never_authenticate() {
        assert_eq!(verify("", ""), Verified::Failed);
        assert_eq!(verify("anything", ""), Verified::Failed);
    }

    #[test]
    fn recognises_its_own_output_as_hashed() {
        assert!(is_hashed(&hash("whatever").unwrap()));
        assert!(!is_hashed("plaintext"));
        assert!(!is_hashed(""));
    }
}
