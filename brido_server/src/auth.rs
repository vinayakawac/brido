//! Session token lifecycle and PIN brute-force protection.
//!
//! Everything here is local and in-process — no external services.
//!
//! Tokens are stored keyed by their SHA-256 hash, never in plaintext, so the
//! on-disk trusted-device file is useless to anyone who reads it. Session
//! tokens live in memory only; trusted-device tokens are persisted so that
//! "skip PIN on future connections" survives a server restart.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::{Mutex, RwLock};

/// Ordinary session tokens expire after this long.
pub const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);
/// Tokens issued to a device the user explicitly trusted.
pub const TRUSTED_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Failed PIN attempts tolerated before an address is locked out.
const FAILURES_BEFORE_LOCKOUT: u32 = 5;
/// Base lockout, doubled for each further failure, capped at [`MAX_LOCKOUT`].
const BASE_LOCKOUT: Duration = Duration::from_secs(5);
const MAX_LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Failure counters older than this are forgotten.
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// SHA-256 of a token, lowercase hex. Tokens are only ever stored like this.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex(&digest)
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Session,
    Trusted,
}

#[derive(Clone)]
struct TokenRecord {
    expires_at: u64,
    kind: TokenKind,
}

impl TokenRecord {
    fn is_expired(&self, now: u64) -> bool {
        self.expires_at <= now
    }
}

#[derive(Default)]
struct Throttle {
    failures: u32,
    last_failure: Option<Instant>,
    locked_until: Option<Instant>,
}

/// Result of a PIN check, so the handler can pick the right status code.
pub enum PinOutcome {
    Ok,
    Invalid,
    /// Locked out; caller should return 429 and suggest waiting this long.
    LockedOut { retry_after: Duration },
}

pub struct AuthState {
    tokens: RwLock<HashMap<String, TokenRecord>>,
    throttle: Mutex<HashMap<IpAddr, Throttle>>,
    trusted_store: PathBuf,
}

impl AuthState {
    /// Loads any previously trusted devices from disk. A missing or malformed
    /// file is not an error — it just means no devices are trusted yet.
    pub fn new(trusted_store: PathBuf) -> Self {
        let tokens = load_trusted(&trusted_store);
        if !tokens.is_empty() {
            tracing::info!("Loaded {} trusted device(s)", tokens.len());
        }
        Self {
            tokens: RwLock::new(tokens),
            throttle: Mutex::new(HashMap::new()),
            trusted_store,
        }
    }

    /// Checks a submitted PIN in constant time, applying per-address throttling.
    ///
    /// A correct PIN clears that address's failure history.
    pub async fn check_pin(&self, peer: IpAddr, submitted: &str, expected: &str) -> PinOutcome {
        let now = Instant::now();
        {
            let mut throttle = self.throttle.lock().await;
            let entry = throttle.entry(peer).or_default();

            if let Some(until) = entry.locked_until {
                if until > now {
                    return PinOutcome::LockedOut {
                        retry_after: until.saturating_duration_since(now),
                    };
                }
                entry.locked_until = None;
            }

            // Forget stale failures so an honest user isn't punished forever.
            if let Some(last) = entry.last_failure {
                if now.saturating_duration_since(last) > FAILURE_WINDOW {
                    entry.failures = 0;
                }
            }
        }

        let matches: bool = submitted
            .as_bytes()
            .ct_eq(expected.as_bytes())
            .unwrap_u8()
            == 1;

        let mut throttle = self.throttle.lock().await;
        let entry = throttle.entry(peer).or_default();

        if matches {
            entry.failures = 0;
            entry.last_failure = None;
            entry.locked_until = None;
            return PinOutcome::Ok;
        }

        entry.failures = entry.failures.saturating_add(1);
        entry.last_failure = Some(now);

        if entry.failures >= FAILURES_BEFORE_LOCKOUT {
            let over = entry.failures - FAILURES_BEFORE_LOCKOUT;
            let lockout = BASE_LOCKOUT
                .saturating_mul(1u32 << over.min(12))
                .min(MAX_LOCKOUT);
            entry.locked_until = Some(now + lockout);
            tracing::warn!(
                "PIN lockout for {peer} after {} failures ({}s)",
                entry.failures,
                lockout.as_secs()
            );
            return PinOutcome::LockedOut {
                retry_after: lockout,
            };
        }

        PinOutcome::Invalid
    }

    /// Issues a new token. Trusted tokens are persisted so they outlive a restart.
    pub async fn issue_token(&self, kind: TokenKind) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let ttl = match kind {
            TokenKind::Session => SESSION_TTL,
            TokenKind::Trusted => TRUSTED_TTL,
        };
        let record = TokenRecord {
            expires_at: now_unix() + ttl.as_secs(),
            kind,
        };

        {
            let mut tokens = self.tokens.write().await;
            tokens.retain(|_, r| !r.is_expired(now_unix()));
            tokens.insert(hash_token(&token), record);
        }

        if kind == TokenKind::Trusted {
            self.persist_trusted().await;
        }

        token
    }

    /// True if the token is known and unexpired. Expired entries are dropped.
    pub async fn verify(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        let key = hash_token(token);
        let now = now_unix();

        {
            let tokens = self.tokens.read().await;
            match tokens.get(&key) {
                Some(record) if !record.is_expired(now) => return true,
                Some(_) => {}
                None => return false,
            }
        }

        // Expired — clean it up.
        let mut tokens = self.tokens.write().await;
        if let Some(record) = tokens.get(&key) {
            if record.is_expired(now) {
                let was_trusted = record.kind == TokenKind::Trusted;
                tokens.remove(&key);
                drop(tokens);
                if was_trusted {
                    self.persist_trusted().await;
                }
            }
        }
        false
    }

    /// Revokes a single token (used by `/api/disconnect`).
    pub async fn revoke(&self, token: &str) -> bool {
        let key = hash_token(token);
        let removed = {
            let mut tokens = self.tokens.write().await;
            tokens.remove(&key)
        };
        match removed {
            Some(record) => {
                if record.kind == TokenKind::Trusted {
                    self.persist_trusted().await;
                }
                true
            }
            None => false,
        }
    }

    /// Drops every expired token. Called periodically by the sweeper task.
    pub async fn sweep(&self) {
        let now = now_unix();
        let mut removed_trusted = false;
        {
            let mut tokens = self.tokens.write().await;
            let before = tokens.len();
            tokens.retain(|_, r| {
                let keep = !r.is_expired(now);
                if !keep && r.kind == TokenKind::Trusted {
                    removed_trusted = true;
                }
                keep
            });
            if tokens.len() != before {
                tracing::debug!("Swept {} expired token(s)", before - tokens.len());
            }
        }
        if removed_trusted {
            self.persist_trusted().await;
        }

        // Throttle entries for addresses that have gone quiet.
        let now_instant = Instant::now();
        let mut throttle = self.throttle.lock().await;
        throttle.retain(|_, t| {
            let locked = t.locked_until.map(|u| u > now_instant).unwrap_or(false);
            let recent = t
                .last_failure
                .map(|l| now_instant.saturating_duration_since(l) <= FAILURE_WINDOW)
                .unwrap_or(false);
            locked || recent
        });
    }

    async fn persist_trusted(&self) {
        let snapshot: Vec<(String, u64)> = {
            let tokens = self.tokens.read().await;
            tokens
                .iter()
                .filter(|(_, r)| r.kind == TokenKind::Trusted)
                .map(|(k, r)| (k.clone(), r.expires_at))
                .collect()
        };

        let body = snapshot
            .iter()
            .map(|(hash, exp)| format!("{hash} {exp}"))
            .collect::<Vec<_>>()
            .join("\n");

        if let Some(parent) = self.trusted_store.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&self.trusted_store, body) {
            tracing::warn!("Could not persist trusted devices: {e}");
        }
    }

    /// Forgets every trusted device (exposed through the desktop UI).
    pub async fn revoke_all_trusted(&self) -> usize {
        let count = {
            let mut tokens = self.tokens.write().await;
            let before = tokens.len();
            tokens.retain(|_, r| r.kind != TokenKind::Trusted);
            before - tokens.len()
        };
        self.persist_trusted().await;
        count
    }
}

/// Reads `hash expiry` lines, skipping anything expired or unparseable.
fn load_trusted(path: &Path) -> HashMap<String, TokenRecord> {
    let mut out = HashMap::new();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return out;
    };
    let now = now_unix();

    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(expiry)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(expires_at) = expiry.parse::<u64>() else {
            continue;
        };
        if expires_at <= now || hash.len() != 64 {
            continue;
        }
        out.insert(
            hash.to_string(),
            TokenRecord {
                expires_at,
                kind: TokenKind::Trusted,
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("brido-test-{name}"))
    }

    #[tokio::test]
    async fn valid_pin_passes_and_wrong_pin_fails() {
        let auth = AuthState::new(temp_store("pin"));
        let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(matches!(
            auth.check_pin(peer, "123456", "123456").await,
            PinOutcome::Ok
        ));
        assert!(matches!(
            auth.check_pin(peer, "000000", "123456").await,
            PinOutcome::Invalid
        ));
    }

    #[tokio::test]
    async fn repeated_failures_lock_the_address_out() {
        let auth = AuthState::new(temp_store("lockout"));
        let peer = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        for _ in 0..FAILURES_BEFORE_LOCKOUT {
            auth.check_pin(peer, "bad", "good").await;
        }
        // Even the correct PIN is refused while locked out.
        assert!(matches!(
            auth.check_pin(peer, "good", "good").await,
            PinOutcome::LockedOut { .. }
        ));
    }

    #[tokio::test]
    async fn tokens_verify_then_revoke() {
        let auth = AuthState::new(temp_store("tokens"));
        let token = auth.issue_token(TokenKind::Session).await;
        assert!(auth.verify(&token).await);
        assert!(auth.revoke(&token).await);
        assert!(!auth.verify(&token).await);
        assert!(!auth.verify("not-a-token").await);
    }

    #[tokio::test]
    async fn trusted_tokens_survive_a_restart() {
        let path = temp_store("trusted");
        let _ = std::fs::remove_file(&path);

        let token = {
            let auth = AuthState::new(path.clone());
            auth.issue_token(TokenKind::Trusted).await
        };

        // Fresh instance reading the same file — as if the server restarted.
        let reloaded = AuthState::new(path.clone());
        assert!(reloaded.verify(&token).await);

        reloaded.revoke_all_trusted().await;
        assert!(!reloaded.verify(&token).await);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn session_tokens_are_not_persisted() {
        let path = temp_store("session-only");
        let _ = std::fs::remove_file(&path);

        let token = {
            let auth = AuthState::new(path.clone());
            auth.issue_token(TokenKind::Session).await
        };

        let reloaded = AuthState::new(path.clone());
        assert!(!reloaded.verify(&token).await);
        let _ = std::fs::remove_file(&path);
    }
}
