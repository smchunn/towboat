//! `towboat status` — show per-file state.

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::manifest::SystemManifest;
use crate::deploy::lock::{FileState, LockFile};
use crate::deploy::symlink;
use crate::resolve::resolver::compute_hash;

pub fn run(stow_dir: &Path, target_dir: &Path, package_filter: Option<&str>) -> Result<()> {
    let manifest_path = stow_dir.join("towboat.toml");
    let manifest = SystemManifest::load(&manifest_path).context("Failed to load towboat.toml")?;

    let towboat_dir = stow_dir.join(".towboat");
    let resolved_dir = towboat_dir.join("resolved");
    let lock_path = towboat_dir.join("towboat.lock");
    let lock = LockFile::load(&lock_path).unwrap_or_default();

    if lock.files.is_empty() {
        println!("No files tracked. Run `towboat sync` first.");
        return Ok(());
    }

    let packages: Vec<&str> = if let Some(name) = package_filter {
        if !manifest.packages.contains_key(name) {
            anyhow::bail!("Package '{name}' not found in towboat.toml");
        }
        vec![name]
    } else {
        manifest.packages.keys().map(|s| s.as_str()).collect()
    };

    let mut any_output = false;

    for pkg_name in &packages {
        let entries = lock.entries_for_package(pkg_name);
        if entries.is_empty() {
            continue;
        }

        println!("Package: {pkg_name}");
        any_output = true;

        for entry in entries {
            let source_path = stow_dir.join(&entry.source);
            let resolved_path = resolved_dir.join(
                entry
                    .source
                    .strip_prefix(&format!("{pkg_name}/"))
                    .map(|s| format!("{pkg_name}/{s}"))
                    .unwrap_or_else(|| entry.source.clone()),
            );
            let link_path = target_dir.join(&entry.target);

            let state = determine_state(
                &source_path,
                &resolved_path,
                &link_path,
                &entry.source_hash,
                &entry.resolved_hash,
            );

            let icon = match &state {
                FileState::UpToDate => "  ",
                FileState::SourceChanged => "M ",
                FileState::Drifted => " D",
                FileState::Conflict => "MD",
                FileState::Broken => "! ",
                FileState::Stale => "S ",
                FileState::New => "N ",
            };

            let label = match &state {
                FileState::UpToDate => "up to date",
                FileState::SourceChanged => "source changed",
                FileState::Drifted => "drifted (resolved file edited)",
                FileState::Conflict => "CONFLICT (both changed)",
                FileState::Broken => "broken symlink",
                FileState::Stale => "stale (removed from config)",
                FileState::New => "new",
            };

            println!("  {icon} {}: {label}", entry.target);
        }
        println!();
    }

    // Check for packages in lock but not in manifest
    let manifest_packages: std::collections::HashSet<&str> = packages.into_iter().collect();
    let lock_packages: std::collections::HashSet<String> =
        lock.files.iter().map(|e| e.package.clone()).collect();

    for pkg in &lock_packages {
        if !manifest_packages.contains(pkg.as_str()) && package_filter.is_none() {
            println!("Package: {pkg} (removed from manifest)");
            for entry in lock.entries_for_package(pkg) {
                println!("  S  {}: stale", entry.target);
            }
            println!();
            any_output = true;
        }
    }

    if !any_output {
        println!("No tracked files for the specified package(s).");
    }

    Ok(())
}

fn determine_state(
    source_path: &Path,
    resolved_path: &Path,
    link_path: &Path,
    locked_source_hash: &str,
    locked_resolved_hash: &str,
) -> FileState {
    // Check for broken symlink first
    if symlink::is_broken_symlink(link_path) {
        return FileState::Broken;
    }

    // Check if source still exists
    if !source_path.exists() {
        return FileState::Stale;
    }

    // Get current hashes
    let current_source_hash = match std::fs::read_to_string(source_path) {
        Ok(content) => compute_hash(&content),
        Err(_) => return FileState::Broken,
    };

    let current_resolved_hash = match std::fs::read_to_string(resolved_path) {
        Ok(content) => compute_hash(&content),
        Err(_) => return FileState::Broken,
    };

    let source_changed = current_source_hash != locked_source_hash;
    let resolved_changed = current_resolved_hash != locked_resolved_hash;

    match (source_changed, resolved_changed) {
        (false, false) => FileState::UpToDate,
        (true, false) => FileState::SourceChanged,
        (false, true) => FileState::Drifted,
        (true, true) => FileState::Conflict,
    }
}
