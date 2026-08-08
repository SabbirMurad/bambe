use std::{env, fs::File, io::BufReader};
use rustls_pemfile::{certs, pkcs8_private_keys as pkcs8};
use rustls::{self, Certificate, PrivateKey, ServerConfig};


pub fn init() -> ServerConfig {
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth();

    // Load TLS key and cert files
    let cert_file = &mut BufReader::new(
        File::open(env::var("TLS_CERT")
            .expect("TLS_CERT must be set on .env file")
        )
        .expect("TLS certificate file not found!")
    );

    let key_file = &mut BufReader::new(
        File::open(env::var("TLS_KEY")
            .expect("TLS_KEY must be set on .env file")
        )
        .expect("TLS private key file not found!")
    );

    // Convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();

    let mut keys: Vec<PrivateKey> = pkcs8(key_file)
        .unwrap()
        .into_iter()
        .map(PrivateKey)
        .collect();

    // Exit if no keys could be parsed
    if keys.is_empty() {
        eprintln!("Couldn't locate PKCS 8 private keys.");
        std::process::exit(1);
    }

    config.with_single_cert(cert_chain, keys.remove(0)).unwrap()
}