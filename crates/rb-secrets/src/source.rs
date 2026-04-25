use std::{env, fs, path::PathBuf};

use crate::{SecretError, SecretValue};

/// Loads secrets from a backing store.
pub trait SecretSource: Send + Sync {
    /// Retrieves the secret identified by `key`.
    ///
    /// # Errors
    ///
    /// Returns [`SecretError::NotFound`] if the key is absent in the backing store,
    /// or [`SecretError::Io`] if the file source cannot read the secret file.
    fn get(&self, key: &str) -> Result<SecretValue, SecretError>;
}

/// Reads secrets from environment variables.
///
/// Key lookup: `{PREFIX}_{KEY}` (both uppercased). When `prefix` is empty the
/// key is looked up directly.
pub struct EnvSource {
    prefix: String,
}

impl EnvSource {
    /// Creates a new `EnvSource` with the given prefix.
    #[must_use]
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl SecretSource for EnvSource {
    fn get(&self, key: &str) -> Result<SecretValue, SecretError> {
        let env_key = if self.prefix.is_empty() {
            key.to_uppercase()
        } else {
            format!("{}_{}", self.prefix.to_uppercase(), key.to_uppercase())
        };
        env::var(&env_key)
            .map(SecretValue::new)
            .map_err(|_| SecretError::NotFound {
                key: env_key.clone(),
            })
    }
}

/// Reads secrets from files in a directory (compatible with Docker secrets).
///
/// Key lookup: reads the file `{dir}/{key}`, trimming trailing whitespace.
pub struct FileSource {
    dir: PathBuf,
}

impl FileSource {
    /// Creates a new `FileSource` rooted at `dir`.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

impl SecretSource for FileSource {
    fn get(&self, key: &str) -> Result<SecretValue, SecretError> {
        let path = self.dir.join(key);
        let content = fs::read_to_string(&path).map_err(|source| SecretError::Io {
            key: key.to_owned(),
            source,
        })?;
        Ok(SecretValue::new(content.trim_end().to_owned()))
    }
}
