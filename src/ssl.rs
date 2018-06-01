extern crate rustls;

extern crate webpki;

extern crate webpki_roots;

use std::sync::Arc;
use std::fs;
use std::io::BufReader;

pub fn setup(cert: Option<&str>) -> Arc<rustls::ClientConfig> {
    let mut config = rustls::ClientConfig::new();
    if let Some(cert) = cert {
        info!("Using {} as cert file", cert);
        let mut pem = BufReader::new(fs::File::open(cert).unwrap());
        config.root_store.add_pem_file(&mut pem).unwrap();
    } else {
        info!("Using TLS server roots for cert");
        config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    }
    Arc::new(config)
}
