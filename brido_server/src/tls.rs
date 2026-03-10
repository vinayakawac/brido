use rcgen::{generate_simple_self_signed, CertifiedKey};

pub struct TlsCert {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

/// Generate a self-signed TLS certificate for the given local IP address.
pub fn generate_self_signed_cert(ip: &str) -> TlsCert {
    let subject_alt_names = vec![
        ip.to_string(),
        "localhost".to_string(),
        "127.0.0.1".to_string(),
    ];

    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(subject_alt_names).expect("Failed to generate TLS certificate");

    TlsCert {
        cert_pem: cert.pem().into_bytes(),
        key_pem: key_pair.serialize_pem().into_bytes(),
    }
}
