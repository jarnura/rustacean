use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::json;

use crate::error::GhError;

/// Mints a short-lived App JWT for authenticating as the GitHub App itself.
///
/// Claims: `iat = now-60s` (GitHub allows 60s clock skew), `exp = now+9min`
/// (GitHub's maximum is 10 min), `iss = app_id`.
pub fn mint_app_jwt(app_id: i64, key: &EncodingKey) -> Result<String, GhError> {
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "iat": now - 60,
        "exp": now + 9 * 60,
        "iss": app_id.to_string(),
    });
    let token = jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, key)?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that mint_app_jwt returns a three-part JWT string without
    /// panicking when given a valid RSA key.
    #[test]
    fn mint_produces_three_part_jwt() {
        // 512-bit test key — insecure, for unit test only.
        let pem = include_str!("../tests/fixtures/test_rsa_512.pem");
        let key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .expect("test key should parse");
        let jwt = mint_app_jwt(12345, &key).expect("mint should succeed");
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT must have three dot-separated parts");
    }
}
