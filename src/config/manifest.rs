//! Parsing for the system manifest (`towboat.toml`).
//!
//! The manifest lives at the root of the stow directory and declares:
//! - Active system tags
//! - Template variables
//! - Which packages to deploy (with optional per-package tag overrides)

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::package::{DefaultConfig, PackageConfig, TargetConfig};
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
///
/// Optionally embeds full `PackageConfig` fields inline, making `boat.toml` optional.
/// If inline config is provided, no `boat.toml` should exist (error if both present).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PackageEntry {
    /// Additional tags required for this package (ANDed with system tags).
    #[serde(default)]
    pub tags: Vec<String>,

    // --- Inline PackageConfig fields (optional) ---
    /// Override target directory for this package (supports `~` expansion).
    #[serde(default)]
    pub target_dir: Option<String>,

    /// Default build tags for this package.
    #[serde(default)]
    pub build_tags: Option<Vec<String>>,

    /// Unified target configurations for files and directories.
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,

    /// Default behavior for unconfigured files.
    #[serde(default)]
    pub default: Option<DefaultConfig>,
}

impl PackageEntry {
    /// Returns `true` if any inline `PackageConfig` fields are set.
    pub fn has_inline_config(&self) -> bool {
        self.target_dir.is_some()
            || self.build_tags.is_some()
            || !self.targets.is_empty()
            || self.default.is_some()
    }

    /// Convert the inline fields to a `PackageConfig`, if any are set.
    pub fn to_package_config(&self) -> Option<PackageConfig> {
        if !self.has_inline_config() {
            return None;
        }
        Some(PackageConfig {
            target_dir: self.target_dir.clone(),
            build_tags: self.build_tags.clone(),
            targets: self.targets.clone(),
            default: self.default.clone(),
        })
    }
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

    #[test]
    fn parse_inline_package_config() {
        let toml_str = r#"
[system]
tags = ["macos", "nvim"]

[packages.home]
target_dir = "~"

[packages.home.targets]
".bashrc" = { tags = ["macos"] }
".vimrc" = { tags = ["macos", "linux"] }
"#;
        let manifest: SystemManifest = toml::from_str(toml_str).unwrap();
        let home = &manifest.packages["home"];
        assert!(home.has_inline_config());
        assert_eq!(home.target_dir, Some("~".to_string()));
        assert_eq!(home.targets.len(), 2);

        let config = home.to_package_config().unwrap();
        assert_eq!(config.target_dir, Some("~".to_string()));
        assert_eq!(config.targets.len(), 2);
    }

    #[test]
    fn parse_mixed_packages() {
        let toml_str = r#"
[system]
tags = ["linux"]

[packages]
bash = {}
vim = { tags = ["development"] }

[packages.home]
target_dir = "~"

[packages.home.targets]
".bashrc" = { tags = ["linux"] }
"#;
        let manifest: SystemManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.packages.len(), 3);

        assert!(!manifest.packages["bash"].has_inline_config());
        assert!(!manifest.packages["vim"].has_inline_config());
        assert!(manifest.packages["home"].has_inline_config());
    }

    #[test]
    fn has_inline_config_detection() {
        let empty = PackageEntry::default();
        assert!(!empty.has_inline_config());

        let with_target_dir = PackageEntry {
            target_dir: Some("~".to_string()),
            ..Default::default()
        };
        assert!(with_target_dir.has_inline_config());

        let with_targets = PackageEntry {
            targets: {
                let mut m = HashMap::new();
                m.insert(
                    ".bashrc".to_string(),
                    TargetConfig {
                        target: None,
                        tags: crate::config::package::TagsSpec::List(vec!["linux".to_string()]),
                    },
                );
                m
            },
            ..Default::default()
        };
        assert!(with_targets.has_inline_config());

        let with_default = PackageEntry {
            default: Some(DefaultConfig {
                include_all: true,
                default_tag: "default".to_string(),
            }),
            ..Default::default()
        };
        assert!(with_default.has_inline_config());
    }

    #[test]
    fn to_package_config_conversion() {
        let entry = PackageEntry {
            tags: vec!["work".to_string()],
            target_dir: Some("~".to_string()),
            build_tags: Some(vec!["production".to_string()]),
            targets: {
                let mut m = HashMap::new();
                m.insert(
                    ".bashrc".to_string(),
                    TargetConfig {
                        target: None,
                        tags: crate::config::package::TagsSpec::List(vec!["linux".to_string()]),
                    },
                );
                m
            },
            default: Some(DefaultConfig {
                include_all: true,
                default_tag: "default".to_string(),
            }),
        };

        let config = entry.to_package_config().unwrap();
        assert_eq!(config.target_dir, Some("~".to_string()));
        assert_eq!(config.build_tags, Some(vec!["production".to_string()]));
        assert_eq!(config.targets.len(), 1);
        assert!(config.default.unwrap().include_all);
    }

    #[test]
    fn to_package_config_none_when_empty() {
        let entry = PackageEntry::default();
        assert!(entry.to_package_config().is_none());
    }
}
