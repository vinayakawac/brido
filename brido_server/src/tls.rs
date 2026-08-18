use rcgen::{generate_simple_self_signed, CertifiedKey};
use sha2::{Digest, Sha256};

pub struct TlsCert {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    /// Lowercase hex SHA-256 of the DER-encoded leaf certificate.
    ///
    /// The app pins against this, so a self-signed certificate still rules out
    /// a man-in-the-middle on the local network. It travels in the QR payload.
    pub fingerprint: String,
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

    let fingerprint = crate::auth::hex(&Sha256::digest(cert.der()));

    TlsCert {
        cert_pem: cert.pem().into_bytes(),
        key_pem: key_pair.serialize_pem().into_bytes(),
        fingerprint,
    }
}
