use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secret string value that zeroes its memory on drop.
///
/// `Debug` is intentionally redacted (`[REDACTED]`) so log statements that
/// accidentally include a `SecretValue` will never leak the plaintext.
/// `Display` is not implemented at all.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretValue(String);

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue([REDACTED])")
    }
}

impl SecretValue {
    pub(crate) fn new(s: String) -> Self {
        Self(s)
    }

    /// Returns the inner secret string.
    ///
    /// Callers should use this value immediately rather than storing it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}
