//! Parsing for per-package configuration (`boat.toml`).
//!
//! Each package directory contains a `boat.toml` that declares:
//! - Optional target directory override
//! - Default build tags for the package
//! - A `[targets]` map of source paths → tag expressions + optional target remapping
//! - Default behavior for unconfigured files

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, TowboatError};

/// Parsed `boat.toml` configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageConfig {
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

/// Configuration for a single file or directory in `[targets]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetConfig {
    /// Optional remapped target path (relative to target directory).
    #[serde(default)]
    pub target: Option<String>,

    /// Tag expression string. Evaluated against active tags.
    /// Can be a simple tag name or a boolean expression (e.g. `"linux & laptop"`).
    #[serde(default)]
    pub tags: TagsSpec,
}

/// How tags are specified in boat.toml — either a list (legacy) or a single expression string.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TagsSpec {
    /// A single tag expression string: `tags = "linux & laptop"`
    Expr(String),
    /// A list of tag names (ORed together): `tags = ["linux", "macos"]`
    List(Vec<String>),
}

impl Default for TagsSpec {
    fn default() -> Self {
        TagsSpec::List(vec![])
    }
}

impl TagsSpec {
    /// Convert to a tag expression string suitable for parsing by `tags::matcher`.
    /// A list `["a", "b"]` becomes `"a | b"`.
    pub fn to_expr_string(&self) -> String {
        match self {
            TagsSpec::Expr(s) => s.clone(),
            TagsSpec::List(tags) => {
                if tags.is_empty() {
                    String::new()
                } else {
                    tags.join(" | ")
                }
            }
        }
    }

    /// Returns true if no tags are specified.
    pub fn is_empty(&self) -> bool {
        match self {
            TagsSpec::Expr(s) => s.is_empty(),
            TagsSpec::List(v) => v.is_empty(),
        }
    }
}

/// Default behavior for files not explicitly listed in `[targets]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultConfig {
    /// If true, include all files not explicitly listed.
    #[serde(default)]
    pub include_all: bool,

    /// Tag to assign to unlisted files when `include_all` is true.
    #[serde(default = "default_tag")]
    pub default_tag: String,
}

fn default_tag() -> String {
    "default".to_string()
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            include_all: false,
            default_tag: default_tag(),
        }
    }
}

impl PackageConfig {
    /// Load and parse a `boat.toml` from the given path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| TowboatError::PackageConfigNotFound(path.to_path_buf()))?;

        toml::from_str(&content).map_err(|e| {
            TowboatError::LockCorrupt(format!(
                "failed to parse boat.toml at {}: {e}",
                path.display()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_boat_config() {
        let toml_str = r#"
target_dir = "~"
build_tags = ["production"]

[targets]
".bashrc" = { tags = ["production", "development"] }
".vimrc" = { target = ".vimrc", tags = ["production"] }
"dev-profile.sh" = { target = "profile.sh", tags = ["development"] }
"scripts" = { tags = ["production", "development"] }

[default]
include_all = false
default_tag = "default"
"#;
        let config: PackageConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.target_dir, Some("~".to_string()));
        assert_eq!(config.build_tags, Some(vec!["production".to_string()]));
        assert_eq!(config.targets.len(), 4);
        assert!(config.default.is_some());
        assert!(!config.default.unwrap().include_all);
    }

    #[test]
    fn parse_with_tag_expressions() {
        let toml_str = r#"
[targets]
".bashrc" = { tags = "linux & laptop" }
".profile" = { tags = "macos | default" }
"scripts" = { tags = ["linux", "macos"] }
"#;
        let config: PackageConfig = toml::from_str(toml_str).unwrap();

        let bashrc = &config.targets[".bashrc"];
        assert_eq!(bashrc.tags.to_expr_string(), "linux & laptop");

        let profile = &config.targets[".profile"];
        assert_eq!(profile.tags.to_expr_string(), "macos | default");

        let scripts = &config.targets["scripts"];
        assert_eq!(scripts.tags.to_expr_string(), "linux | macos");
    }

    #[test]
    fn parse_minimal_config() {
        let toml_str = r#"
[targets]
".bashrc" = { tags = ["default"] }
"#;
        let config: PackageConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.target_dir, None);
        assert_eq!(config.build_tags, None);
        assert_eq!(config.targets.len(), 1);
        assert!(config.default.is_none());
    }

    #[test]
    fn tags_spec_expr_string() {
        assert_eq!(
            TagsSpec::Expr("linux & laptop".into()).to_expr_string(),
            "linux & laptop"
        );
        assert_eq!(
            TagsSpec::List(vec!["a".into(), "b".into()]).to_expr_string(),
            "a | b"
        );
        assert_eq!(TagsSpec::List(vec![]).to_expr_string(), "");
    }

    #[test]
    fn tags_spec_is_empty() {
        assert!(TagsSpec::List(vec![]).is_empty());
        assert!(TagsSpec::Expr(String::new()).is_empty());
        assert!(!TagsSpec::List(vec!["a".into()]).is_empty());
        assert!(!TagsSpec::Expr("linux".into()).is_empty());
    }

    #[test]
    fn target_with_remap() {
        let toml_str = r#"
[targets]
"dev-profile.sh" = { target = "profile.sh", tags = ["development"] }
"#;
        let config: PackageConfig = toml::from_str(toml_str).unwrap();
        let entry = &config.targets["dev-profile.sh"];
        assert_eq!(entry.target, Some("profile.sh".to_string()));
    }

    #[test]
    fn default_config_defaults() {
        let config = DefaultConfig::default();
        assert!(!config.include_all);
        assert_eq!(config.default_tag, "default");
    }
}
