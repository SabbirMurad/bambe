use std::{env, fs::File, io::BufReader};
use rustls_pemfile::{certs, pkcs8_private_keys};
use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;


pub fn init() -> ServerConfig {
    // rustls 0.23 supports pluggable crypto backends and no longer auto-selects
    // one when more than one of its own provider features (aws-lc-rs, ring) ends
    // up compiled in across the dependency graph — it panics at the first
    // ServerConfig::builder() call until a process-wide default is installed.
    // `Err` here just means one was already installed (harmless, so ignored
    // rather than unwrapped).
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

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
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to parse TLS certificate file!");

    let mut keys: Vec<PrivateKeyDer<'static>> = pkcs8_private_keys(key_file)
        .map(|k| k.map(PrivateKeyDer::from))
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to parse TLS private key file!");

    // Exit if no keys could be parsed
    if keys.is_empty() {
        eprintln!("Couldn't locate PKCS 8 private keys.");
        std::process::exit(1);
    }

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, keys.remove(0))
        .unwrap()
}
