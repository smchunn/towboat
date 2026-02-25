//! `towboat diff` — show what would change on next sync.

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::manifest::SystemManifest;
use crate::deploy::lock::LockFile;
use crate::resolve::resolver::{compute_hash, resolve_file};

pub fn run(stow_dir: &Path, _target_dir: &Path, package_filter: Option<&str>) -> Result<()> {
    let manifest_path = stow_dir.join("towboat.toml");
    let manifest = SystemManifest::load(&manifest_path).context("Failed to load towboat.toml")?;

    let active_tags = manifest.active_tags();
    let towboat_dir = stow_dir.join(".towboat");
    let resolved_dir = towboat_dir.join("resolved");
    let lock_path = towboat_dir.join("towboat.lock");
    let lock = LockFile::load(&lock_path).unwrap_or_default();

    let packages: Vec<(&str, &crate::config::manifest::PackageEntry)> =
        if let Some(name) = package_filter {
            match manifest.packages.get(name) {
                Some(entry) => vec![(name, entry)],
                None => anyhow::bail!("Package '{name}' not found in towboat.toml"),
            }
        } else {
            manifest
                .packages
                .iter()
                .map(|(k, v)| (k.as_str(), v))
                .collect()
        };

    let mut any_changes = false;

    for (pkg_name, pkg_entry) in &packages {
        let pkg_dir = stow_dir.join(pkg_name);
        if !pkg_dir.exists() {
            continue;
        }

        let config = crate::config::resolve_package_config(pkg_name, &pkg_dir, pkg_entry)?;

        let discovered =
            crate::discovery::walker::discover_package(&pkg_dir, &config, &active_tags)?;

        for file in &discovered {
            let source_relative = format!("{pkg_name}/{}", file.relative_path.display());

            let (new_content, _had_tags) =
                resolve_file(&file.source_path, &active_tags, &manifest.variables)?;

            let new_hash = compute_hash(&new_content);

            // Check against lock
            if let Some(lock_entry) = lock.find(pkg_name, &source_relative) {
                if new_hash != lock_entry.resolved_hash {
                    // Content would change
                    if !any_changes {
                        println!("Changes that would be applied on next sync:\n");
                        any_changes = true;
                    }
                    println!(
                        "  M {pkg_name}/{} -> {}",
                        file.relative_path.display(),
                        file.target_path.display()
                    );

                    // Show basic diff info
                    let resolved_path = resolved_dir
                        .join(pkg_name)
                        .join(file.relative_path.to_string_lossy().as_ref());
                    if resolved_path.exists() {
                        let old_content = std::fs::read_to_string(&resolved_path)?;
                        let old_lines: Vec<&str> = old_content.lines().collect();
                        let new_lines: Vec<&str> = new_content.lines().collect();
                        print_simple_diff(&old_lines, &new_lines);
                    }
                }
            } else {
                // New file
                if !any_changes {
                    println!("Changes that would be applied on next sync:\n");
                    any_changes = true;
                }
                println!(
                    "  + {pkg_name}/{} -> {} (new)",
                    file.relative_path.display(),
                    file.target_path.display()
                );
            }
        }

        // Check for files that would be removed
        let current_sources: std::collections::HashSet<String> = discovered
            .iter()
            .map(|f| format!("{pkg_name}/{}", f.relative_path.display()))
            .collect();

        for entry in lock.entries_for_package(pkg_name) {
            if !current_sources.contains(&entry.source) {
                if !any_changes {
                    println!("Changes that would be applied on next sync:\n");
                    any_changes = true;
                }
                println!("  - {} (would be removed)", entry.target);
            }
        }
    }

    if !any_changes {
        println!("No changes would be applied.");
    }

    Ok(())
}

fn print_simple_diff(old: &[&str], new: &[&str]) {
    // Simple line-by-line comparison
    let max_lines = old.len().max(new.len());
    let mut showed_diff = false;

    for i in 0..max_lines {
        let old_line = old.get(i).copied();
        let new_line = new.get(i).copied();

        match (old_line, new_line) {
            (Some(o), Some(n)) if o != n => {
                println!("    - {o}");
                println!("    + {n}");
                showed_diff = true;
            }
            (Some(o), None) => {
                println!("    - {o}");
                showed_diff = true;
            }
            (None, Some(n)) => {
                println!("    + {n}");
                showed_diff = true;
            }
            _ => {}
        }
    }

    if showed_diff {
        println!();
    }
}
