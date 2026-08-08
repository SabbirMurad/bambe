use std::env;
use std::time::Duration;
use mongodb::{ Client, ClientSession, Database, error };
use mongodb::options::{ ClientOptions, ServerAddress };

pub struct MongoDB;
impl MongoDB {
  fn init(&self) -> Result<Client, error::Error> {
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

    Ok(Client::with_options(options)?)
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
