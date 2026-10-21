use std::env;
use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;

/*
  For Loading Dynamic Database
  Add / Remove fields based on your project needs!
*/
pub enum DBF { IMG, JWT }

impl DBF {
    /// The `.env` variable holding this database's file path.
    fn env_key(&self) -> &'static str {
        match self {
            DBF::IMG => "SQLITE_IMG_PATH",
            DBF::JWT => "SQLITE_JWT_PATH",
        }
    }

    fn cell(&self) -> &'static OnceLock<SqlitePool> {
        static IMG_POOL: OnceLock<SqlitePool> = OnceLock::new();
        static JWT_POOL: OnceLock<SqlitePool> = OnceLock::new();

        match self {
            DBF::IMG => &IMG_POOL,
            DBF::JWT => &JWT_POOL,
        }
    }
}

pub type SqlitePool = Pool<SqliteConnectionManager>;
pub type SqliteConn = PooledConnection<SqliteConnectionManager>;

/// Errors from the SQLite layer. Wraps both the driver's errors and the
/// pool's, so callers get one type instead of two.
#[derive(Debug)]
pub enum Error {
    /// A required `.env` variable is missing.
    Config(String),
    /// Checking a connection out of the pool failed (usually a timeout because
    /// every connection is busy).
    Pool(r2d2::Error),
    /// The query itself failed.
    Sqlite(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Config(message) => write!(fmt, "{}", message),
            Error::Pool(error) => write!(fmt, "sqlite pool: {}", error),
            Error::Sqlite(error) => write!(fmt, "sqlite: {}", error),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self { Error::Sqlite(error) }
}

impl From<r2d2::Error> for Error {
    fn from(error: r2d2::Error) -> Self { Error::Pool(error) }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Connections a single database may hand out at once. SQLite serialises
/// writers regardless, so this mainly bounds concurrent readers under WAL.
const MAX_POOL_SIZE: u32 = 8;

/// How long a request waits for a free connection before giving up.
const CHECKOUT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long SQLite retries a locked table before returning SQLITE_BUSY.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds the pool for one database file.
///
/// `with_init` runs on every physical connection the pool opens, which is the
/// only place these pragmas take effect — they are per-connection, not stored
/// in the file (except `journal_mode`, which is persistent but harmless to
/// re-apply).
fn build_pool(path: &str) -> Result<SqlitePool> {
    let manager = SqliteConnectionManager::file(path).with_init(|conn| {
        // `execute_batch` tolerates statements that return rows, which
        // `PRAGMA journal_mode` does — `execute`/`pragma_update` would error.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        Ok(())
    });

    Ok(Pool::builder()
        .max_size(MAX_POOL_SIZE)
        .connection_timeout(CHECKOUT_TIMEOUT)
        .build(manager)?)
}

/// The process-wide pool for `dbf`, built on first use.
///
/// Two threads racing here may each build a pool; the loser's is dropped
/// immediately and closes its connections. Harmless, and cheaper than holding
/// a lock across the build.
fn pool(dbf: &DBF) -> Result<&'static SqlitePool> {
    let cell = dbf.cell();

    if let Some(pool) = cell.get() {
        return Ok(pool);
    }

    let key = dbf.env_key();
    let path = env::var(key)
        .map_err(|_| Error::Config(format!("{} must be set on .env file", key)))?;

    let built = build_pool(&path)?;
    Ok(cell.get_or_init(|| built))
}

/// Checks a pooled connection out for the caller.
///
/// Returns it to the pool on drop — do not hold one across an `.await`, or the
/// connection stays checked out for the whole await and starves other requests.
pub fn connect(dbf: DBF) -> Result<SqliteConn> {
    Ok(pool(&dbf)?.get()?)
}

pub fn create_initial_tables() -> Result<()> {
    /* Following table is for storing images */
    let db_conn = connect(DBF::IMG)?;
    /* Chat voice notes / audio blobs (Bambee). */
    db_conn.execute(
        "CREATE TABLE IF NOT EXISTS audio (
            uuid          TEXT PRIMARY KEY,
            data          BLOB NOT NULL,
            mime          TEXT NOT NULL
        );", ()
    )?;
    db_conn.execute(
        "CREATE TABLE IF NOT EXISTS image (
            uuid          TEXT PRIMARY KEY,
            original      BLOB NOT NULL,
            webp          BLOB NOT NULL
        );", ()
    )?;

    /*
      Following table is for JWT Refresh Tokens - one row per SESSION, not per
      user.

      It used to be keyed `issuer TEXT PRIMARY KEY`, so an account could hold
      exactly one refresh token: signing in on a phone silently invalidated the
      laptop, and rotating on one device signed out every other. For a chat app
      that is the wrong default, since people genuinely use more than one device.

      Keyed on `token` instead, with an index on `issuer`, so a user has as many
      live sessions as devices. That is also what makes "sign out this device"
      (block one row) and "sign out everywhere" (block all rows for an issuer)
      expressible at all - neither was possible with one row per user.
    */
    let db_conn = connect(DBF::JWT)?;

    /*
      Rebuild rather than migrate when an old-shaped table is found.

      The column order changed, and refresh tokens are disposable: the worst case
      is that everyone signs in again. That is a far better outcome than a table
      whose columns silently no longer mean what the queries assume, which is
      what `CREATE TABLE IF NOT EXISTS` alone would leave behind on any database
      created before this change.
    */
    let legacy = db_conn
        .prepare("SELECT name FROM pragma_table_info('refreshToken') ORDER BY cid LIMIT 1")
        .and_then(|mut stmt| stmt.query_row([], |row| row.get::<usize, String>(0)))
        .map(|first_column| first_column != "token")
        .unwrap_or(false);

    if legacy {
        log::warn!(
            "refreshToken table has the old one-row-per-user shape; rebuilding for per-device sessions. Everyone will need to sign in again."
        );
        db_conn.execute("DROP TABLE refreshToken;", ())?;
    }

    db_conn.execute(
        "CREATE TABLE IF NOT EXISTS refreshToken (
            token           TEXT PRIMARY KEY,
            issuer          TEXT NOT NULL,
            status          TEXT NOT NULL,
            created_at      INTEGER NOT NULL,
            modified_at     INTEGER
        );", ()
    )?;

    // Every per-user operation - list this account's sessions, sign out
    // everywhere, prune on sign-in - filters by issuer, which is no longer the
    // primary key. Without this each of those scans the whole table.
    db_conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_refreshToken_issuer
         ON refreshToken (issuer);", ()
    )?;

    Ok(())
}
