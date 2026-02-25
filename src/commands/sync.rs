//! `towboat sync` — resolve packages, create/update symlinks, update lock file.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::config::manifest::SystemManifest;
use crate::deploy::lock::{FileState, LockEntry, LockFile};
use crate::deploy::symlink;
use crate::discovery::walker;
use crate::resolve::resolver::{compute_hash, resolve_file};

pub fn run(
    stow_dir: &Path,
    target_dir: &Path,
    package_filter: Option<&str>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let manifest_path = stow_dir.join("towboat.toml");
    let manifest = SystemManifest::load(&manifest_path)
        .context("Failed to load towboat.toml — run `towboat init` to create one")?;

    let active_tags = manifest.active_tags();
    let towboat_dir = stow_dir.join(".towboat");
    let resolved_dir = towboat_dir.join("resolved");
    let lock_path = towboat_dir.join("towboat.lock");

    let mut lock = LockFile::load(&lock_path).unwrap_or_default();
    let mut stats = SyncStats::default();
    let mut conflicts = Vec::new();

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

    for (pkg_name, pkg_entry) in &packages {
        let pkg_dir = stow_dir.join(pkg_name);
        if !pkg_dir.exists() {
            eprintln!(
                "Warning: package directory not found: {}",
                pkg_dir.display()
            );
            continue;
        }

        let config = crate::config::resolve_package_config(pkg_name, &pkg_dir, pkg_entry)?;

        // Check package-level tag requirements
        if !pkg_entry.tags.is_empty() && !pkg_entry.tags.iter().all(|t| active_tags.contains(t)) {
            cleanup_package(&mut lock, pkg_name, target_dir, &mut stats, dry_run)?;
            continue;
        }

        // Discover files (without resolving yet)
        let discovered = walker::discover_package(&pkg_dir, &config, &active_tags)?;
        let mut seen_sources: HashSet<String> = HashSet::new();

        for file in &discovered {
            let source_relative = format!("{pkg_name}/{}", file.relative_path.display());
            seen_sources.insert(source_relative.clone());

            let resolved_path = resolved_dir.join(pkg_name).join(&file.relative_path);
            let link_path = target_dir.join(&file.target_path);

            // Compute current source hash
            let source_content = std::fs::read_to_string(&file.source_path).with_context(|| {
                format!("Failed to read source: {}", file.source_path.display())
            })?;
            let source_hash = compute_hash(&source_content);

            // Check drift BEFORE resolving (using the old resolved file on disk)
            if let Some(lock_entry) = lock.find(pkg_name, &source_relative) {
                let old_resolved_hash = if resolved_path.exists() {
                    let content = std::fs::read_to_string(&resolved_path)?;
                    compute_hash(&content)
                } else {
                    String::new()
                };

                let state = lock_entry.state(&source_hash, &old_resolved_hash);
                match state {
                    FileState::UpToDate => {
                        // Just ensure symlink is correct
                        if !symlink::symlink_matches(&link_path, &resolved_path) {
                            if dry_run {
                                println!(
                                    "Would fix symlink: {} -> {}",
                                    link_path.display(),
                                    resolved_path.display()
                                );
                            } else {
                                symlink::create_symlink(&resolved_path, &link_path, force)?;
                            }
                            stats.symlinks_created += 1;
                        }
                        continue;
                    }
                    FileState::Conflict if !force => {
                        conflicts.push(format!(
                            "{source_relative}: source and resolved both changed since last sync"
                        ));
                        continue;
                    }
                    FileState::Drifted if !force => {
                        // Source hasn't changed — preserve user's edits
                        continue;
                    }
                    _ => {
                        // SourceChanged, force, or new — proceed to resolve
                    }
                }
            }

            // Now resolve (tags + templates)
            let (resolved_content, _had_tags) =
                match resolve_file(&file.source_path, &active_tags, &manifest.variables) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Error: {}: {e}", file.relative_path.display());
                        stats.errors += 1;
                        continue;
                    }
                };

            let resolved_hash = compute_hash(&resolved_content);

            // Write resolved file
            if let Some(parent) = resolved_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&resolved_path, &resolved_content)?;
            stats.resolved += 1;

            // Create/update symlink
            // Always force-overwrite our own symlinks (file is tracked in lock or new)
            let is_our_symlink = lock.find(pkg_name, &source_relative).is_some()
                || symlink::symlink_matches(&link_path, &resolved_path);
            let effective_force = force || is_our_symlink;

            if dry_run {
                println!(
                    "Would symlink: {} -> {}",
                    link_path.display(),
                    resolved_path.display()
                );
            } else {
                symlink::create_symlink(&resolved_path, &link_path, effective_force)?;
            }
            stats.symlinks_created += 1;

            // Update lock entry
            if !dry_run {
                lock.upsert(LockEntry {
                    package: pkg_name.to_string(),
                    source: source_relative,
                    source_hash,
                    resolved_hash,
                    target: file.target_path.to_string_lossy().to_string(),
                    tags_matched: vec![file.matched_expr.clone()],
                });
            }
        }

        // Clean up stale entries for this package
        if !dry_run {
            cleanup_stale_entries(
                &mut lock,
                pkg_name,
                &seen_sources,
                target_dir,
                &resolved_dir,
                &mut stats,
            )?;
        }
    }

    // Clean up packages removed from manifest (only when syncing all)
    if package_filter.is_none() && !dry_run {
        cleanup_removed_packages(&manifest, &mut lock, target_dir, &mut stats)?;
    }

    // Save lock file
    if !dry_run {
        lock.last_sync = Some(Utc::now());
        lock.save(&lock_path)?;
    }

    print_summary(&stats, &conflicts, dry_run);

    if !conflicts.is_empty() && !force {
        anyhow::bail!(
            "{} conflict(s) detected. Use --force to overwrite.",
            conflicts.len()
        );
    }

    Ok(())
}

fn cleanup_package(
    lock: &mut LockFile,
    pkg_name: &str,
    target_dir: &Path,
    stats: &mut SyncStats,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    let stale = lock.entries_for_package(pkg_name);
    for entry in &stale {
        let link_path = target_dir.join(&entry.target);
        if link_path.is_symlink() || link_path.exists() {
            symlink::remove_symlink(&link_path)?;
            stats.stale_removed += 1;
        }
    }
    lock.remove_package(pkg_name);
    Ok(())
}

fn cleanup_stale_entries(
    lock: &mut LockFile,
    pkg_name: &str,
    seen_sources: &HashSet<String>,
    target_dir: &Path,
    resolved_dir: &Path,
    stats: &mut SyncStats,
) -> Result<()> {
    let stale_entries: Vec<LockEntry> = lock
        .entries_for_package(pkg_name)
        .into_iter()
        .filter(|e| !seen_sources.contains(&e.source))
        .cloned()
        .collect();

    for entry in &stale_entries {
        let link_path = target_dir.join(&entry.target);
        if link_path.is_symlink() || link_path.exists() {
            symlink::remove_symlink(&link_path)?;
            stats.stale_removed += 1;
        }
        let resolved_file = resolved_dir.join(pkg_name).join(
            entry
                .source
                .strip_prefix(&format!("{pkg_name}/"))
                .unwrap_or(&entry.source),
        );
        if resolved_file.exists() {
            std::fs::remove_file(&resolved_file).ok();
        }
    }

    lock.files
        .retain(|e| e.package != pkg_name || seen_sources.contains(&e.source));
    Ok(())
}

fn cleanup_removed_packages(
    manifest: &SystemManifest,
    lock: &mut LockFile,
    target_dir: &Path,
    stats: &mut SyncStats,
) -> Result<()> {
    let manifest_packages: HashSet<&str> = manifest.packages.keys().map(|s| s.as_str()).collect();
    let lock_packages: HashSet<String> = lock.files.iter().map(|e| e.package.clone()).collect();

    for pkg in &lock_packages {
        if !manifest_packages.contains(pkg.as_str()) {
            let stale = lock.entries_for_package(pkg);
            for entry in &stale {
                let link_path = target_dir.join(&entry.target);
                if link_path.is_symlink() || link_path.exists() {
                    symlink::remove_symlink(&link_path)?;
                    stats.stale_removed += 1;
                }
            }
            lock.remove_package(pkg);
        }
    }
    Ok(())
}

#[derive(Default)]
struct SyncStats {
    resolved: usize,
    symlinks_created: usize,
    stale_removed: usize,
    errors: usize,
}

fn print_summary(stats: &SyncStats, conflicts: &[String], dry_run: bool) {
    let prefix = if dry_run { "Would: " } else { "" };

    if stats.resolved > 0 {
        println!("{prefix}{} file(s) resolved", stats.resolved);
    }
    if stats.symlinks_created > 0 {
        println!(
            "{prefix}{} symlink(s) created/updated",
            stats.symlinks_created
        );
    }
    if stats.stale_removed > 0 {
        println!("{prefix}{} stale entry(ies) removed", stats.stale_removed);
    }
    if stats.errors > 0 {
        eprintln!("{} error(s) occurred", stats.errors);
    }
    for conflict in conflicts {
        eprintln!("Conflict: {conflict}");
    }
    if stats.resolved == 0
        && stats.symlinks_created == 0
        && stats.stale_removed == 0
        && conflicts.is_empty()
        && stats.errors == 0
    {
        println!("Everything up to date.");
    }
}
