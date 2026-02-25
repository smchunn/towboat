//! File resolution: reads source, processes tags, substitutes templates, writes to resolved dir.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::package::PackageConfig;
use crate::discovery::walker::{self, DiscoveredFile};
use crate::error::Result;
use crate::resolve::{ResolveOutcome, ResolvedFile};
use crate::tags::parser;
use crate::template::engine;

/// Compute SHA256 hash of content.
pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Resolve a single file: read source, process tags, substitute templates.
pub fn resolve_file(
    source_path: &Path,
    active_tags: &HashSet<String>,
    variables: &HashMap<String, String>,
) -> Result<(String, bool)> {
    let content = fs::read_to_string(source_path)?;

    // Step 1: Process build tag sections
    let parsed = parser::process_tags(&content, active_tags)?;

    // Step 2: Substitute template variables
    let resolved = engine::render(&parsed.content, variables)?;

    Ok((resolved, parsed.had_tags))
}

/// Resolve all files in a package: discover, process, and write to resolved directory.
///
/// Returns the resolve outcome and writes resolved files to `resolved_dir`.
pub fn resolve_package(
    package_name: &str,
    package_dir: &Path,
    config: &PackageConfig,
    active_tags: &HashSet<String>,
    variables: &HashMap<String, String>,
    resolved_dir: &Path,
) -> Result<ResolveOutcome> {
    let discovered = walker::discover_package(package_dir, config, active_tags)?;
    let mut outcome = ResolveOutcome::default();

    for file in discovered {
        match resolve_and_write(
            package_name,
            package_dir,
            &file,
            active_tags,
            variables,
            resolved_dir,
        ) {
            Ok(resolved) => outcome.resolved.push(resolved),
            Err(e) => outcome
                .errors
                .push(format!("{}: {}", file.relative_path.display(), e)),
        }
    }

    Ok(outcome)
}

/// Resolve a single discovered file and write it to the resolved directory.
fn resolve_and_write(
    package_name: &str,
    _package_dir: &Path,
    file: &DiscoveredFile,
    active_tags: &HashSet<String>,
    variables: &HashMap<String, String>,
    resolved_dir: &Path,
) -> Result<ResolvedFile> {
    let (content, had_tags) = resolve_file(&file.source_path, active_tags, variables)?;

    // Write to resolved directory: .towboat/resolved/<package>/<relative_path>
    let resolved_path = resolved_dir.join(package_name).join(&file.relative_path);

    if let Some(parent) = resolved_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&resolved_path, &content)?;

    // Source relative to stow directory (parent of package dir)
    let source_relative = PathBuf::from(package_name).join(&file.relative_path);

    Ok(ResolvedFile {
        package: package_name.to_string(),
        source_relative,
        content,
        target_relative: file.target_path.clone(),
        had_tags,
        matched_expr: file.matched_expr.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tags(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn compute_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2);
        assert_ne!(compute_hash("hello world"), compute_hash("hello world!"));
    }

    #[test]
    fn resolve_file_with_tags() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.sh");
        fs::write(
            &file,
            "\
#!/bin/bash
# {linux-
echo linux
# -linux}
# {macos-
echo macos
# -macos}
echo common
",
        )
        .unwrap();

        let (content, had_tags) = resolve_file(&file, &tags(&["linux"]), &vars(&[])).unwrap();
        assert!(had_tags);
        assert!(content.contains("echo linux"));
        assert!(!content.contains("echo macos"));
        assert!(content.contains("echo common"));
    }

    #[test]
    fn resolve_file_with_templates() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("config");
        fs::write(&file, "host = {{ hostname }}\nemail = {{ email }}").unwrap();

        let (content, had_tags) = resolve_file(
            &file,
            &tags(&[]),
            &vars(&[("hostname", "mybox"), ("email", "me@example.com")]),
        )
        .unwrap();

        assert!(!had_tags);
        assert_eq!(content, "host = mybox\nemail = me@example.com");
    }

    #[test]
    fn resolve_file_tags_then_templates() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("config");
        fs::write(
            &file,
            "\
host = {{ hostname }}
# {linux-
path = /usr/bin
# -linux}
# {macos-
path = /usr/local/bin
# -macos}
",
        )
        .unwrap();

        let (content, had_tags) =
            resolve_file(&file, &tags(&["linux"]), &vars(&[("hostname", "mybox")])).unwrap();

        assert!(had_tags);
        assert!(content.contains("host = mybox"));
        assert!(content.contains("path = /usr/bin"));
        assert!(!content.contains("/usr/local/bin"));
    }

    #[test]
    fn resolve_package_full() {
        let stow_dir = TempDir::new().unwrap();
        let pkg_dir = stow_dir.path().join("bash");
        fs::create_dir_all(&pkg_dir).unwrap();

        fs::write(
            pkg_dir.join("boat.toml"),
            r#"
[targets]
".bashrc" = { tags = ["linux"] }
".profile" = { tags = ["linux"] }
"#,
        )
        .unwrap();

        fs::write(
            pkg_dir.join(".bashrc"),
            "\
# {linux-
alias ls='ls --color=auto'
# -linux}
export PATH=$PATH
",
        )
        .unwrap();

        fs::write(pkg_dir.join(".profile"), "source ~/.bashrc\n").unwrap();

        let config = PackageConfig::load(&pkg_dir.join("boat.toml")).unwrap();
        let resolved_dir = stow_dir.path().join(".towboat/resolved");

        let outcome = resolve_package(
            "bash",
            &pkg_dir,
            &config,
            &tags(&["linux"]),
            &vars(&[]),
            &resolved_dir,
        )
        .unwrap();

        assert_eq!(outcome.resolved.len(), 2);
        assert!(outcome.errors.is_empty());

        // Check resolved files were written
        let resolved_bashrc = resolved_dir.join("bash/.bashrc");
        assert!(resolved_bashrc.exists());
        let content = fs::read_to_string(resolved_bashrc).unwrap();
        assert!(content.contains("--color=auto"));
        assert!(content.contains("export PATH=$PATH"));

        let resolved_profile = resolved_dir.join("bash/.profile");
        assert!(resolved_profile.exists());
    }

    #[test]
    fn resolve_package_with_templates() {
        let stow_dir = TempDir::new().unwrap();
        let pkg_dir = stow_dir.path().join("git");
        fs::create_dir_all(&pkg_dir).unwrap();

        fs::write(
            pkg_dir.join("boat.toml"),
            r#"
[targets]
".gitconfig" = { tags = ["default"] }
"#,
        )
        .unwrap();

        fs::write(
            pkg_dir.join(".gitconfig"),
            "[user]\n    name = {{ git_name }}\n    email = {{ git_email }}\n",
        )
        .unwrap();

        let config = PackageConfig::load(&pkg_dir.join("boat.toml")).unwrap();
        let resolved_dir = stow_dir.path().join(".towboat/resolved");

        let outcome = resolve_package(
            "git",
            &pkg_dir,
            &config,
            &tags(&["default"]),
            &vars(&[("git_name", "Alice"), ("git_email", "alice@example.com")]),
            &resolved_dir,
        )
        .unwrap();

        assert_eq!(outcome.resolved.len(), 1);
        let content = fs::read_to_string(resolved_dir.join("git/.gitconfig")).unwrap();
        assert!(content.contains("name = Alice"));
        assert!(content.contains("email = alice@example.com"));
    }

    #[test]
    fn resolve_package_undefined_variable_collected_as_error() {
        let stow_dir = TempDir::new().unwrap();
        let pkg_dir = stow_dir.path().join("pkg");
        fs::create_dir_all(&pkg_dir).unwrap();

        fs::write(
            pkg_dir.join("boat.toml"),
            r#"
[targets]
"config" = { tags = ["default"] }
"#,
        )
        .unwrap();

        fs::write(pkg_dir.join("config"), "host = {{ undefined_var }}").unwrap();

        let config = PackageConfig::load(&pkg_dir.join("boat.toml")).unwrap();
        let resolved_dir = stow_dir.path().join(".towboat/resolved");

        let outcome = resolve_package(
            "pkg",
            &pkg_dir,
            &config,
            &tags(&["default"]),
            &vars(&[]),
            &resolved_dir,
        )
        .unwrap();

        assert_eq!(outcome.resolved.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(outcome.errors[0].contains("undefined_var"));
    }
}
