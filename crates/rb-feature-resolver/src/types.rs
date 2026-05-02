use std::collections::BTreeSet;

/// The set of features explicitly requested by the caller, plus a flag for default-feature
/// handling.
///
/// Build with [`FeatureSet::default`] (which enables default features) or customise via the
/// builder methods.
#[derive(Debug, Clone, Default)]
pub struct FeatureSet {
    features: BTreeSet<String>,
    no_default_features: bool,
}

impl FeatureSet {
    /// Request `features` in addition to (or instead of, if `no_default_features` is set) the
    /// crate's declared defaults.
    #[must_use]
    pub fn with_features(features: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            features: features.into_iter().map(Into::into).collect(),
            no_default_features: false,
        }
    }

    /// Disable the `default` feature; only the explicitly listed features will be resolved.
    #[must_use]
    pub fn no_default_features(mut self) -> Self {
        self.no_default_features = true;
        self
    }

    /// Returns `true` if the `default` feature should be included in resolution.
    #[must_use]
    pub fn uses_default_features(&self) -> bool {
        !self.no_default_features
    }

    /// The explicitly requested feature names.
    #[must_use]
    pub fn features(&self) -> &BTreeSet<String> {
        &self.features
    }
}

/// The canonical, fully-resolved set of features to pass to `cargo expand --features`.
///
/// All transitive feature dependencies are expanded; `default` is included when the input
/// `FeatureSet` did not opt out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedFeatureSet {
    features: BTreeSet<String>,
}

impl ResolvedFeatureSet {
    pub(crate) fn new(features: BTreeSet<String>) -> Self {
        Self { features }
    }

    /// The resolved feature names, sorted.
    #[must_use]
    pub fn features(&self) -> &BTreeSet<String> {
        &self.features
    }

    /// Iterate over resolved feature names as `&str`.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.features.iter().map(String::as_str)
    }

    /// Return the feature list as `--features` argument fragments, e.g. `["foo", "bar"]`.
    #[must_use]
    pub fn as_cargo_args(&self) -> Vec<&str> {
        self.features.iter().map(String::as_str).collect()
    }

    /// Returns `true` when no features are resolved.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}
