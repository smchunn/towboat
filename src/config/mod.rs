pub mod manifest;
pub mod package;

use std::path::Path;

use anyhow::{Context, Result};

/// Resolve the `PackageConfig` for a package using the precedence rules:
///
/// | Inline config? | `boat.toml` exists? | Behavior                         |
/// |----------------|---------------------|----------------------------------|
/// | Yes            | No                  | Use inline                       |
/// | No             | Yes                 | Use `boat.toml` (backwards compat)|
/// | No             | No                  | Default (`include_all: true`)    |
/// | Yes            | Yes                 | **Error** — pick one             |
pub fn resolve_package_config(
    pkg_name: &str,
    pkg_dir: &Path,
    pkg_entry: &manifest::PackageEntry,
) -> Result<package::PackageConfig> {
    let boat_toml = pkg_dir.join("boat.toml");
    let has_boat_toml = boat_toml.exists();
    let has_inline = pkg_entry.has_inline_config();

    match (has_inline, has_boat_toml) {
        (true, true) => {
            anyhow::bail!(
                "Package '{pkg_name}' has both inline config in towboat.toml and a boat.toml file. \
                 Remove one — only a single source of configuration is allowed per package."
            );
        }
        (true, false) => {
            // Safe to unwrap: has_inline_config() returned true
            Ok(pkg_entry.to_package_config().unwrap())
        }
        (false, true) => {
            package::PackageConfig::load(&boat_toml).with_context(|| {
                format!(
                    "Failed to load boat.toml for package '{pkg_name}' at {}",
                    pkg_dir.display()
                )
            })
        }
        (false, false) => Ok(package::PackageConfig {
            target_dir: None,
            build_tags: None,
            targets: std::collections::HashMap::new(),
            default: Some(package::DefaultConfig {
                include_all: true,
                default_tag: "default".to_string(),
            }),
        }),
    }
}
