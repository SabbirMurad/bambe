use std::env;
use std::sync::OnceLock;
use std::time::Duration;
use mongodb::{ Client, ClientSession, Database, error };
use mongodb::options::ClientOptions;

/*
  The process-wide MongoDB client.

  A `Client` owns a connection pool AND a background topology-monitoring task.
  `connect()` used to build a fresh one on every call, at all 37 call sites, so a
  single request that touched Mongo twice created two clients, opened up to
  `min_pool_size` * 2 connections, ran two queries, then tore the whole thing
  down. That is connection churn, file descriptors and latency on every request.

  `Client` is internally reference-counted, so handing out clones from here is
  cheap and every caller shares one pool.

  Built once in `main` rather than lazily, because `ClientOptions::parse` is
  async and `connect()` is called from 37 synchronous sites. Parsing at startup
  keeps those sites unchanged and turns a malformed connection string into a
  startup failure instead of a surprise on the first request that needs it.
*/
static CLIENT: OnceLock<Client> = OnceLock::new();

/*
  Whether this deployment can run multi-document transactions.

  MongoDB rejects them outright on a standalone `mongod` - the server replies
  "Transaction numbers are only allowed on a replica set member or mongos" to the
  first operation carrying a transaction number. Not to `start_transaction`,
  which is purely client-side, which is why the old code appeared to work: it
  started transactions that no operation ever joined, so nothing ever carried a
  transaction number and nothing ever complained.

  Now that sessions are actually threaded through (#9), that reply would arrive.
  Rather than make a replica set a hard requirement for the template to boot, a
  transaction is opened only where one can succeed.

  Derived from the parsed connection string rather than read separately, so the
  topology and the transaction decision cannot disagree.
*/
static TRANSACTIONS: OnceLock<bool> = OnceLock::new();

pub struct MongoDB;
impl MongoDB {
  /*
    Builds the shared client. Call once, from `main`, before serving.

    Everything about the connection comes from `MONGO_URI`, so credentials,
    `authSource`, TLS, replica set and Atlas (`mongodb+srv://`) are all config
    rather than code:

      mongodb://127.0.0.1:27017
      mongodb://user:pass@127.0.0.1:27017/?authSource=admin&replicaSet=rs0
      mongodb+srv://user:pass@cluster.mongodb.net/

    This replaced a `MONGO_HOST` + `MONGO_PORT` pair that could express none of
    the above - a project pointing at a managed database had to edit this file.
  */
  pub async fn init() -> Result<(), error::Error> {
    let uri = env::var("MONGO_URI")
      .expect("MONGO_URI must be set on .env file");

    let mut options = ClientOptions::parse(&uri).await?;

    let app_name = env!("CARGO_PKG_NAME");

    /*
      Defaults, applied only where the connection string did not say. Anything
      given as a URI parameter wins, which is the point of moving to a URI - the
      operator should not have to come back here to change a pool size.
    */
    if options.app_name.is_none() {
      options.app_name = Some(app_name.into());
    }

    // The database may be named in the URI path (mongodb://host/mydb). Falling
    // back to the crate name preserves the previous behaviour, but the URI now
    // lets a project pick a name without renaming the crate.
    if options.default_database.is_none() {
      options.default_database = Some(app_name.into());
    }

    // The driver default is 30s, which turns an outage into 30-second request
    // hangs rather than a fast failure.
    if options.server_selection_timeout.is_none() {
      options.server_selection_timeout = Some(Duration::from_secs(5));
    }

    if options.max_idle_time.is_none() {
      options.max_idle_time = Some(Duration::from_secs(30));
    }

    // Totals for the process, not a per-request allocation.
    if options.min_pool_size.is_none() {
      options.min_pool_size = Some(8);
    }

    if options.max_pool_size.is_none() {
      options.max_pool_size = Some(256);
    }

    let replica_set = options.repl_set_name.clone();

    match &replica_set {
      Some(name) => {
        // Topology discovery routes to the primary. `direct_connection` must
        // stay off here: it pins the driver to one node and reports the topology
        // as Single, which is not a deployment that supports transactions.
        log::info!("MongoDB: replica set \"{}\" (transactions enabled)", name);
      }
      None => {
        // Talk to exactly this server and skip discovery - correct, and
        // marginally faster, for a single mongod.
        if options.direct_connection.is_none() {
          options.direct_connection = Some(true);
        }

        log::warn!(
          "MongoDB: standalone, running WITHOUT transactions. Multi-document writes (sign-up, sign-in, verification) are not atomic, so a failure part-way through leaves orphaned documents behind. Add ?replicaSet=<name> to MONGO_URI once mongod runs as a replica set."
        );
      }
    }

    let _ = TRANSACTIONS.set(replica_set.is_some());
    let _ = CLIENT.set(Client::with_options(options)?);

    log::info!("MongoDB client created (shared process-wide)");

    Ok(())
  }

  /// The shared client.
  ///
  /// Panics if `init()` has not run. That is a startup-ordering bug rather than
  /// a runtime condition, so it should never be reachable in a served request.
  fn client(&self) -> &'static Client {
    CLIENT
      .get()
      .expect("MongoDB::init() must be awaited in main before serving requests")
  }

  #[allow(dead_code)]
  pub fn connect(&self) -> Database {
    self.client()
      .default_database()
      .expect("Failed to connect with Default Database")
  }

  #[allow(dead_code)]
  pub async fn connect_acid(&self) -> (Database, ClientSession) {
    let client = self.client();
    let db = client.default_database().expect("Failed to connect with Default Database");
    let session = client.start_session().await.expect("Failed to start MongoDB ClientSession");

    (db, session)
  }

  /// Whether `begin` should actually open a transaction - see `TRANSACTIONS`.
  fn transactions_enabled() -> bool {
    *TRANSACTIONS.get().unwrap_or(&false)
  }

  /// Open a transaction, where the deployment supports one.
  ///
  /// On a standalone this is a no-op that returns `Ok`, so callers keep their
  /// existing shape and the writes simply run unprotected - which is exactly
  /// what they did before, just without pretending otherwise.
  pub async fn begin(session: &mut ClientSession) -> Result<(), error::Error> {
    if !Self::transactions_enabled() {
      return Ok(());
    }

    session.start_transaction().await
  }

  /// Commit the transaction opened by `begin`, if there is one.
  pub async fn commit(session: &mut ClientSession) -> Result<(), error::Error> {
    if !Self::transactions_enabled() {
      return Ok(());
    }

    session.commit_transaction().await
  }

  /*
    Roll back an in-progress transaction on an error path.

    The previous idiom at ~50 call sites was

        session.abort_transaction().await.ok().unwrap();

    which is `Option::unwrap` on the result of discarding the error: if the
    abort itself fails - a dropped connection, a stepped-down primary, exactly
    the conditions that make an abort necessary - the handler panics. In an
    actix worker that kills the connection and returns nothing to the client,
    turning a recoverable database error into a hard request failure.

    A failed abort is also not something a handler can act on. The transaction
    will be rolled back regardless: either the server aborts it when the session
    expires, or `ClientSession::drop` spawns an abort. So the only correct
    response is to record it and carry on returning the original error, which is
    what the caller was already about to do.
  */
  pub async fn abort(session: &mut ClientSession) {
    if !Self::transactions_enabled() {
      return;
    }

    if let Err(error) = session.abort_transaction().await {
      log::warn!("failed to abort transaction (it will be rolled back anyway): {:?}", error);
    }
  }

  #[allow(dead_code)]
  pub fn connect_with(&self, db_name: &str) -> Database {
    self.client().database(db_name)
  }
}
