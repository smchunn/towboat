//! Parsing for the system manifest (`towboat.toml`).
//!
//! The manifest lives at the root of the stow directory and declares:
//! - Active system tags
//! - Template variables
//! - Which packages to deploy (with optional per-package tag overrides)

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, TowboatError};

/// Top-level `towboat.toml` manifest.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemManifest {
    #[serde(default)]
    pub system: SystemConfig,

    #[serde(default)]
    pub variables: HashMap<String, String>,

    #[serde(default)]
    pub packages: HashMap<String, PackageEntry>,
}

/// `[system]` section of the manifest.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SystemConfig {
    /// Active build tags for this system (e.g. `["macos", "laptop", "work"]`).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Entry in the `[packages]` table.
///
/// An empty table `bash = {}` means "deploy with system tags".
/// `vim = { tags = ["development"] }` means "only deploy when these extra tags match".
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PackageEntry {
    /// Additional tags required for this package (ANDed with system tags).
    #[serde(default)]
    pub tags: Vec<String>,
}

impl SystemManifest {
    /// Load and parse a `towboat.toml` from the given path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| TowboatError::ManifestNotFound(path.to_path_buf()))?;

        toml::from_str(&content)
            .map_err(|e| TowboatError::ManifestNotFound(path.to_path_buf()).into_io(e))
    }

    /// Collect the full set of active tags as a `HashSet`.
    pub fn active_tags(&self) -> std::collections::HashSet<String> {
        self.system.tags.iter().cloned().collect()
    }
}

// Helper to convert TOML parse errors into IO errors with context.
impl TowboatError {
    fn into_io(self, toml_err: toml::de::Error) -> Self {
        TowboatError::LockCorrupt(format!("failed to parse manifest: {toml_err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_manifest() {
        let toml_str = r#"
[system]
tags = ["macos", "laptop", "work"]

[variables]
hostname = "macbook-pro"
email = "user@work.com"

[packages]
bash = {}
vim = { tags = ["development"] }
ssh = { tags = ["work"] }
"#;
        let manifest: SystemManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.system.tags, vec!["macos", "laptop", "work"]);
        assert_eq!(manifest.variables["hostname"], "macbook-pro");
        assert_eq!(manifest.packages.len(), 3);
        assert!(manifest.packages["bash"].tags.is_empty());
        assert_eq!(manifest.packages["vim"].tags, vec!["development"]);
    }

    #[test]
    fn parse_minimal_manifest() {
        let toml_str = r#"
[system]
tags = ["default"]

[packages]
bash = {}
"#;
        let manifest: SystemManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.system.tags, vec!["default"]);
        assert!(manifest.variables.is_empty());
        assert_eq!(manifest.packages.len(), 1);
    }

    #[test]
    fn active_tags_as_hashset() {
        let toml_str = r#"
[system]
tags = ["macos", "laptop"]

[packages]
bash = {}
"#;
        let manifest: SystemManifest = toml::from_str(toml_str).unwrap();
        let tags = manifest.active_tags();
        assert!(tags.contains("macos"));
        assert!(tags.contains("laptop"));
        assert!(!tags.contains("linux"));
    }

    #[test]
    fn empty_manifest() {
        let toml_str = "";
        let manifest: SystemManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.system.tags.is_empty());
        assert!(manifest.variables.is_empty());
        assert!(manifest.packages.is_empty());
    }
}
