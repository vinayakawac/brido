use std::path::{Path, PathBuf};

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

/// Loads the server's TLS certificate, generating and saving one on first run.
///
/// The certificate is persisted so its fingerprint survives a restart. Without
/// this, every launch produced a new certificate and every phone that had
/// pinned the old one refused to reconnect — which silently broke "trust this
/// device". A cached certificate is reused only when it still covers `ip`.
pub fn load_or_create_cert(dir: &Path, ip: &str) -> TlsCert {
    let cert_path = dir.join("tls_cert.pem");
    let key_path = dir.join("tls_key.pem");
    let ip_path = dir.join("tls_host.txt");

    // Reuse the stored certificate when it was issued for this same address.
    let stored_ip = std::fs::read_to_string(&ip_path).unwrap_or_default();
    if stored_ip.trim() == ip {
        if let (Ok(cert_pem), Ok(key_pem)) = (
            std::fs::read(&cert_path),
            std::fs::read(&key_path),
        ) {
            if let Some(fingerprint) = fingerprint_from_pem(&cert_pem) {
                tracing::info!("Reusing stored TLS certificate");
                return TlsCert {
                    cert_pem,
                    key_pem,
                    fingerprint,
                };
            }
            tracing::warn!("Stored TLS certificate unreadable; regenerating");
        }
    } else if !stored_ip.trim().is_empty() {
        tracing::info!(
            "Local address changed ({} -> {ip}); issuing a new TLS certificate",
            stored_ip.trim()
        );
    }

    let fresh = generate_self_signed_cert(ip);

    // Best effort: if the directory is read-only the server still runs, it just
    // gets a new certificate next time.
    let _ = std::fs::create_dir_all(dir);
    if let Err(e) = std::fs::write(&cert_path, &fresh.cert_pem)
        .and_then(|_| std::fs::write(&key_path, &fresh.key_pem))
        .and_then(|_| std::fs::write(&ip_path, ip))
    {
        tracing::warn!("Could not persist TLS certificate: {e}");
    }

    fresh
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

/// SHA-256 of the DER body inside a PEM certificate.
///
/// Computed from the decoded DER (not the PEM text) so it matches what the
/// phone sees on the wire.
fn fingerprint_from_pem(pem: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(pem).ok()?;
    let body: String = text
        .lines()
        .skip_while(|l| !l.contains("BEGIN CERTIFICATE"))
        .skip(1)
        .take_while(|l| !l.contains("END CERTIFICATE"))
        .collect();
    if body.is_empty() {
        return None;
    }
    let der = base64_decode(&body)?;
    Some(crate::auth::hex(&Sha256::digest(&der)))
}

/// Minimal standard-alphabet base64 decoder for PEM bodies.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    let mut chunk = Vec::with_capacity(4);

    for &b in &cleaned {
        if b == b'=' {
            chunk.push(None);
        } else {
            chunk.push(Some(val(b)?));
        }
        if chunk.len() == 4 {
            let pad = chunk.iter().filter(|c| c.is_none()).count();
            let n = (chunk[0]? << 18)
                | (chunk[1]? << 12)
                | (chunk[2].unwrap_or(0) << 6)
                | chunk[3].unwrap_or(0);
            out.push((n >> 16) as u8);
            if pad < 2 {
                out.push((n >> 8) as u8);
            }
            if pad < 1 {
                out.push(n as u8);
            }
            chunk.clear();
        }
    }

    if chunk.is_empty() {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_survives_a_pem_round_trip() {
        let cert = generate_self_signed_cert("192.168.0.5");
        let from_pem = fingerprint_from_pem(&cert.cert_pem).expect("parses");
        assert_eq!(from_pem, cert.fingerprint);
    }

    #[test]
    fn stored_certificate_is_reused_for_the_same_address() {
        let dir = std::env::temp_dir().join("brido-tls-test-reuse");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let first = load_or_create_cert(&dir, "192.168.0.5");
        let second = load_or_create_cert(&dir, "192.168.0.5");
        // Same fingerprint means a phone that pinned it still connects.
        assert_eq!(first.fingerprint, second.fingerprint);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn address_change_issues_a_new_certificate() {
        let dir = std::env::temp_dir().join("brido-tls-test-newip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let first = load_or_create_cert(&dir, "192.168.0.5");
        let second = load_or_create_cert(&dir, "10.0.0.7");
        assert_ne!(first.fingerprint, second.fingerprint);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
