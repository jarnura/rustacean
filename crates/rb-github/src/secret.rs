/// A wrapper that prevents the inner value from appearing in `Debug` or
/// `Display` output, so secrets are never written to logs or traces.
pub struct Secret<T>(T);

impl<T: Clone> Clone for Secret<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Secret<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<T> std::fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let s = Secret::new("super-secret".to_owned());
        assert_eq!(format!("{s:?}"), "[REDACTED]");
    }

    #[test]
    fn display_is_redacted() {
        let s = Secret::new("super-secret".to_owned());
        assert_eq!(format!("{s}"), "[REDACTED]");
    }

    #[test]
    fn expose_returns_inner() {
        let s = Secret::new(vec![1u8, 2, 3]);
        assert_eq!(s.expose(), &[1u8, 2, 3]);
    }
}
