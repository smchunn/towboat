//! Directory traversal and file discovery.
//!
//! Walks a package directory, consults `boat.toml` configuration, and returns
//! the list of files that should be included for the active tags.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::package::PackageConfig;
use crate::error::Result;
use crate::tags::matcher;

/// A file discovered during directory walking.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    /// Absolute path to the source file.
    pub source_path: PathBuf,
    /// Path relative to the package directory (used as default target).
    pub relative_path: PathBuf,
    /// Final target path (may be remapped via boat.toml `target` field).
    pub target_path: PathBuf,
    /// The tag expression that matched.
    pub matched_expr: String,
}

/// Walk a package directory and return all files matching the active tags.
///
/// Respects `boat.toml` configuration including:
/// - Explicit target entries with tag expressions
/// - Directory tag inheritance
/// - Nested `boat.toml` files (subdirectory precedence)
/// - Default behavior for unconfigured files
pub fn discover_package(
    package_dir: &Path,
    config: &PackageConfig,
    active_tags: &HashSet<String>,
) -> Result<Vec<DiscoveredFile>> {
    let mut results = Vec::new();
    walk_dir(
        package_dir,
        package_dir,
        package_dir,
        config,
        active_tags,
        &mut results,
    )?;
    Ok(results)
}

/// `package_root` — top-level package dir (for computing output relative paths).
/// `config_root` — the directory whose `boat.toml` is in effect (may be a nested subdir).
fn walk_dir(
    package_root: &Path,
    config_root: &Path,
    dir: &Path,
    config: &PackageConfig,
    active_tags: &HashSet<String>,
    results: &mut Vec<DiscoveredFile>,
) -> Result<()> {
    for entry in WalkDir::new(dir)
        .min_depth(if dir == config_root { 1 } else { 0 })
        .max_depth(1)
        .follow_links(false)
    {
        let entry = entry.map_err(|e| std::io::Error::other(format!("walkdir error: {e}")))?;
        let path = entry.path();

        // Skip boat.toml and .towboat directory
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && (name == "boat.toml" || name == ".towboat" || name == "towboat.toml")
        {
            continue;
        }

        if path.is_dir() && path != dir {
            // Check for nested boat.toml — if present, recurse with its own config
            let nested_config_path = path.join("boat.toml");
            if nested_config_path.exists() {
                let nested_config = PackageConfig::load(&nested_config_path)?;
                walk_dir(
                    package_root,
                    path,
                    path,
                    &nested_config,
                    active_tags,
                    results,
                )?;
            } else {
                // Recurse into subdirectory with parent config
                walk_dir(
                    package_root,
                    config_root,
                    path,
                    config,
                    active_tags,
                    results,
                )?;
            }
        } else if path.is_file()
            && let Some(discovered) =
                check_file(package_root, config_root, path, config, active_tags)?
        {
            results.push(discovered);
        }
    }

    Ok(())
}

/// Check if a single file should be included based on config and active tags.
///
/// `package_root` — for computing output `relative_path`.
/// `config_root` — for matching paths against `boat.toml` entries.
fn check_file(
    package_root: &Path,
    config_root: &Path,
    file_path: &Path,
    config: &PackageConfig,
    active_tags: &HashSet<String>,
) -> Result<Option<DiscoveredFile>> {
    // Path relative to package root (used in output)
    let relative = file_path
        .strip_prefix(package_root)
        .map_err(|_| std::io::Error::other("failed to strip prefix"))?;

    // Path relative to config root (used for matching boat.toml entries)
    let config_relative = file_path
        .strip_prefix(config_root)
        .map_err(|_| std::io::Error::other("failed to strip prefix"))?;

    let config_path_str = config_relative.to_string_lossy();

    // 1. Check explicit target entry
    if let Some(target_config) = config.targets.get(config_path_str.as_ref()) {
        let expr_str = target_config.tags.to_expr_string();
        if !expr_str.is_empty() && evaluate_expr(&expr_str, active_tags)? {
            let target_path = target_config
                .target
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| relative.to_path_buf());
            return Ok(Some(DiscoveredFile {
                source_path: file_path.to_path_buf(),
                relative_path: relative.to_path_buf(),
                target_path,
                matched_expr: expr_str,
            }));
        }
        // Explicitly configured but doesn't match — skip
        if !expr_str.is_empty() {
            return Ok(None);
        }
    }

    // 2. Check parent directory entries (tag inheritance)
    let mut check_path: &Path = config_relative;
    while let Some(parent) = check_path.parent() {
        if parent == Path::new("") {
            break;
        }
        let parent_str = parent.to_string_lossy();
        if let Some(parent_config) = config.targets.get(parent_str.as_ref()) {
            let expr_str = parent_config.tags.to_expr_string();
            if !expr_str.is_empty() && evaluate_expr(&expr_str, active_tags)? {
                return Ok(Some(DiscoveredFile {
                    source_path: file_path.to_path_buf(),
                    relative_path: relative.to_path_buf(),
                    target_path: relative.to_path_buf(),
                    matched_expr: expr_str,
                }));
            }
            if !expr_str.is_empty() {
                return Ok(None);
            }
        }
        check_path = parent;
    }

    // 3. Check default behavior
    let defaults = config.default.as_ref().cloned().unwrap_or_default();
    if defaults.include_all {
        let expr_str = defaults.default_tag.clone();
        if evaluate_expr(&expr_str, active_tags)? {
            return Ok(Some(DiscoveredFile {
                source_path: file_path.to_path_buf(),
                relative_path: relative.to_path_buf(),
                target_path: relative.to_path_buf(),
                matched_expr: expr_str,
            }));
        }
    }

    Ok(None)
}

fn evaluate_expr(expr_str: &str, active_tags: &HashSet<String>) -> Result<bool> {
    let expr = matcher::parse(expr_str)?;
    Ok(expr.evaluate(active_tags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_package(files: &[(&str, &str)], config_toml: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("boat.toml"), config_toml).unwrap();

        for (path, content) in files {
            let full_path = dir.path().join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full_path, content).unwrap();
        }

        dir
    }

    fn tags(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn discover_explicit_targets() {
        let dir = setup_package(
            &[(".bashrc", "content"), (".vimrc", "content")],
            r#"
[targets]
".bashrc" = { tags = ["linux"] }
".vimrc" = { tags = ["macos"] }
"#,
        );

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();
        let results = discover_package(dir.path(), &config, &tags(&["linux"])).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relative_path, PathBuf::from(".bashrc"));
    }

    #[test]
    fn discover_with_tag_expression() {
        let dir = setup_package(
            &[(".bashrc", "content")],
            r#"
[targets]
".bashrc" = { tags = "linux & laptop" }
"#,
        );

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();

        let results = discover_package(dir.path(), &config, &tags(&["linux", "laptop"])).unwrap();
        assert_eq!(results.len(), 1);

        let results = discover_package(dir.path(), &config, &tags(&["linux", "desktop"])).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn discover_directory_inheritance() {
        let dir = setup_package(
            &[
                (".config/hypr/hyprland.conf", "hypr config"),
                (".config/hypr/startup.sh", "startup script"),
            ],
            r#"
[targets]
".config/hypr" = { tags = ["linux"] }
"#,
        );

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();
        let results = discover_package(dir.path(), &config, &tags(&["linux"])).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn discover_with_target_remap() {
        let dir = setup_package(
            &[("dev-profile.sh", "content")],
            r#"
[targets]
"dev-profile.sh" = { target = "profile.sh", tags = ["development"] }
"#,
        );

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();
        let results = discover_package(dir.path(), &config, &tags(&["development"])).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target_path, PathBuf::from("profile.sh"));
    }

    #[test]
    fn discover_with_default_include_all() {
        let dir = setup_package(
            &[(".bashrc", "content"), (".profile", "content")],
            r#"
[default]
include_all = true
default_tag = "default"
"#,
        );

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();

        let results = discover_package(dir.path(), &config, &tags(&["default"])).unwrap();
        assert_eq!(results.len(), 2);

        let results = discover_package(dir.path(), &config, &tags(&["linux"])).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn discover_nested_boat_toml() {
        let dir = TempDir::new().unwrap();

        // Root config
        fs::write(
            dir.path().join("boat.toml"),
            r#"
[targets]
".bashrc" = { tags = ["linux"] }
"#,
        )
        .unwrap();

        fs::write(dir.path().join(".bashrc"), "bash content").unwrap();

        // Nested directory with its own config
        let nested_dir = dir.path().join("subpkg");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(
            nested_dir.join("boat.toml"),
            r#"
[targets]
"config.txt" = { tags = ["macos"] }
"#,
        )
        .unwrap();
        fs::write(nested_dir.join("config.txt"), "nested content").unwrap();

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();

        // Only linux active — should get .bashrc but not nested config.txt
        let results = discover_package(dir.path(), &config, &tags(&["linux"])).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relative_path, PathBuf::from(".bashrc"));

        // Only macos active — should get nested config.txt but not .bashrc
        let results = discover_package(dir.path(), &config, &tags(&["macos"])).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relative_path, PathBuf::from("subpkg/config.txt"));
    }

    #[test]
    fn discover_skips_towboat_dir() {
        let dir = setup_package(
            &[(".bashrc", "content")],
            r#"
[default]
include_all = true
default_tag = "default"
"#,
        );

        // Create .towboat directory that should be ignored
        fs::create_dir_all(dir.path().join(".towboat")).unwrap();
        fs::write(
            dir.path().join(".towboat/checksums.toml"),
            "should be ignored",
        )
        .unwrap();

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();
        let results = discover_package(dir.path(), &config, &tags(&["default"])).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relative_path, PathBuf::from(".bashrc"));
    }

    #[test]
    fn discover_no_matches() {
        let dir = setup_package(
            &[(".bashrc", "content")],
            r#"
[targets]
".bashrc" = { tags = ["linux"] }
"#,
        );

        let config = PackageConfig::load(&dir.path().join("boat.toml")).unwrap();
        let results = discover_package(dir.path(), &config, &tags(&["macos"])).unwrap();
        assert_eq!(results.len(), 0);
    }
}
