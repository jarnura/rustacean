// State token helpers — implemented in RUSAA-46 (REQ-GH-02, install URL + callback).
//
// Opaque 256-bit random token for the OAuth-like install flow. The raw token
// is returned to the client; only sha256(token) is stored server-side.

use sha2::{Digest, Sha256};

/// Returns the SHA-256 hex digest of the given token bytes.
#[must_use]
pub fn hash_token(token: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_64_hex_chars() {
        let digest = hash_token(b"test-token-value");
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn same_input_produces_same_hash() {
        assert_eq!(hash_token(b"abc"), hash_token(b"abc"));
    }

    #[test]
    fn different_inputs_produce_different_hashes() {
        assert_ne!(hash_token(b"abc"), hash_token(b"xyz"));
    }
}
