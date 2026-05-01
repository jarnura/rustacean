use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::FeatureResolveError;

/// Parsed representation of a `Cargo.toml` file.
///
/// Only the fields needed for feature resolution are captured; unknown fields are ignored.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CargoManifest {
    /// `[package]` table.
    pub package: Option<PackageMetadata>,

    /// `[features]` table — maps each feature name to the list of features/deps it enables.
    #[serde(default)]
    pub features: BTreeMap<String, Vec<String>>,

    /// `[dependencies]` table.
    #[serde(default)]
    pub dependencies: BTreeMap<String, DependencySpec>,

    /// `[dev-dependencies]` table.
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: BTreeMap<String, DependencySpec>,

    /// `[build-dependencies]` table.
    #[serde(rename = "build-dependencies", default)]
    pub build_dependencies: BTreeMap<String, DependencySpec>,

    /// `[workspace]` table — present when this is a workspace root.
    pub workspace: Option<WorkspaceSection>,
}

/// Metadata from `[package]`.
#[derive(Debug, Clone, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: Option<String>,
}

/// `[workspace]` section as it appears in a workspace-root `Cargo.toml`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WorkspaceSection {
    /// Glob patterns for workspace member directories.
    #[serde(default)]
    pub members: Vec<String>,
}

/// Inline or detailed dependency specification.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Short form: `dep = "1.0"`.
    Simple(String),
    /// Long form: `dep = { version = "1.0", features = ["foo"], ... }`.
    Detailed(DetailedDependency),
}

/// Long-form dependency entry.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DetailedDependency {
    pub version: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(rename = "default-features", default = "detail_default_true")]
    pub default_features: bool,
    #[serde(default)]
    pub optional: bool,
}

fn detail_default_true() -> bool {
    true
}

impl CargoManifest {
    /// Read and parse a `Cargo.toml` file at `path`.
    /// # Errors
    ///
    /// Returns [`FeatureResolveError::ManifestRead`] if the file cannot be read, or
    /// [`FeatureResolveError::ManifestParse`] if the TOML is malformed.
    pub fn from_path(path: &Path) -> Result<Self, FeatureResolveError> {
        let content =
            std::fs::read_to_string(path).map_err(|source| FeatureResolveError::ManifestRead {
                path: path.to_owned(),
                source,
            })?;
        toml::from_str(&content).map_err(|source| FeatureResolveError::ManifestParse {
            path: path.to_owned(),
            source,
        })
    }

    /// The crate's declared package name, if present.
    #[must_use]
    pub fn package_name(&self) -> Option<&str> {
        self.package.as_ref().map(|p| p.name.as_str())
    }
}
