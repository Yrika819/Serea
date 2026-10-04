use std::fmt;

/// Payload-free backend failure. Implementations must discard internal diagnostics
/// before crossing this boundary; neither bytes nor key/envelope details belong here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtRestProtectionError;

impl fmt::Display for AtRestProtectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AtRestProtectionFailed")
    }
}
impl std::error::Error for AtRestProtectionError {}

/// Injected protection for PRIVATE blob payloads only, not ordinary task rows.
///
/// The implementation is trusted to provide actual at-rest protection. Storage
/// cannot determine cryptographic quality or detect a backend returning plaintext.
/// P2D ships no real implementation. Test doubles prove wiring, not security.
///
/// The backend owns its opaque, reversible envelope (including any nonce or key
/// version). Output need not be deterministic. It must not expose internal secret
/// diagnostics through errors, logging or formatting. No Debug bound is required.
/// Calls run synchronously under the Store transaction lock: do not reenter Store.
pub trait AtRestProtection: Send + Sync {
    /// Protects canonical PRIVATE plaintext. Failure must never return plaintext
    /// as a fallback. Successful bytes are stored verbatim, not re-digested.
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, AtRestProtectionError>;

    /// Recovers PRIVATE plaintext from the backend's envelope. Unsupported or
    /// unreadable envelopes must refuse; storage then canonicalizes and checks
    /// the plaintext digest independently before returning any bytes.
    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, AtRestProtectionError>;
}
