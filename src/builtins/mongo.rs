use std::env;
use std::sync::OnceLock;
use std::time::Duration;
use mongodb::{ Client, ClientSession, Database, error };
use mongodb::options::{ ClientOptions, ServerAddress };

/*
  One client for the process, not one per call.

  `Client` owns the connection pool, and every handler calls `connect()` — so
  building a fresh one per request meant each request stood up its own pool
  (`min_pool_size: 8`), used a single connection, and dropped the lot. The
  driver's own docs are explicit that `Client` is meant to be created once and
  shared; it is `Arc` inside, so cloning is cheap and it is `Send + Sync`.

  `OnceLock` keeps the call sites untouched: the same `MongoDB.connect()` reads
  the same signature and returns the same `Database`, it just stops rebuilding
  the pool underneath.
*/
static CLIENT: OnceLock<Client> = OnceLock::new();

pub struct MongoDB;
impl MongoDB {
  fn init(&self) -> Result<Client, error::Error> {
    if let Some(client) = CLIENT.get() {
      return Ok(client.clone());
    }

    let app_name = env!("CARGO_PKG_NAME");
    let host = env::var("MONGO_HOST")
    .expect("MONGO_HOST must be set on .env file");
    let port = env::var("MONGO_PORT")
    .expect("MONGO_PORT must be set on .env file");

    let address = ServerAddress::Tcp {
      host,
      port: Some(port.parse().unwrap()),
    };

    let options = ClientOptions::builder()
    .hosts(vec![address])
    .direct_connection(Some(true))
    .max_idle_time(Some(Duration::new(30, 0)))
    .min_pool_size(Some(8))
    .max_pool_size(Some(256))
    .default_database(Some(app_name.into()))
    .app_name(Some(app_name.into()))
    .build();

    let client = Client::with_options(options)?;

    /*
      Two threads can race to here on the first request. `set` returns the
      loser's client back to it; both are valid, and the loser's is dropped
      after we read whatever ended up stored, so every later caller shares one.
    */
    let _ = CLIENT.set(client);
    Ok(CLIENT.get().expect("client set above").clone())
  }

  #[allow(dead_code)]
  pub fn connect(&self) -> Database {
    let client = self.init().expect("Failed to initiate MongoDB Client");
    client.default_database().expect("Failed to connect with Default Database")
  }

  #[allow(dead_code)]
  pub async fn connect_acid(&self) -> (Database, ClientSession) {
    let client = self.init().expect("Failed to initiate MongoDB Client");
    let db = client.default_database().expect("Failed to connect with Default Database");
    let session = client.start_session().await.expect("Failed to start MongoDB ClientSession");

    (db, session)
  }

  #[allow(dead_code)]
  pub fn connect_with(&self, db_name: &str) -> Database {
    let client = self.init().expect("Failed to initiate MongoDB Client");
    client.database(db_name)
  }
}
