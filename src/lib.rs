//! Towboat - A cross-platform dotfile manager with build tags
//!
//! This crate provides functionality for managing dotfiles across multiple platforms
//! using build tags to include/exclude platform-specific content.
//!
//! # Examples
//!
//! ```rust
//! use towboat::{Config, run_towboat};
//! use std::path::PathBuf;
//!
//! let config = Config {
//!     source_dir: PathBuf::from("./dotfiles/home"),
//!     stow_dir: PathBuf::from("./dotfiles"),
//!     target_dir: PathBuf::from("/home/user"),
//!     build_tag: "linux".to_string(),
//!     dry_run: false,
//!     force: false,
//!     adopt: false,
//!     remove: false,
//! };
//!
//! // This would deploy Linux-specific dotfiles
//! // run_towboat(config).unwrap();
//! ```

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Target configuration from boat.toml
/// Applies to both files and directories
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetConfig {
    /// Target path (relative to target directory)
    /// If not specified, defaults to the source filename/dirname
    #[serde(default)]
    pub target: Option<String>,

    /// Build tags this target should be included for
    pub tags: Vec<String>,
}

/// Default configuration behavior
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultConfig {
    /// Whether to include all files/directories not explicitly configured
    pub include_all: bool,

    /// Default tag to assign to files/directories that are not explicitly configured
    #[serde(default = "default_tag")]
    pub default_tag: String,
}

fn default_tag() -> String {
    "default".to_string()
}

/// boat.toml configuration file structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoatConfig {

    /// Target configurations (files and directories)
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,

    /// Default behavior configuration
    #[serde(default)]
    pub default: Option<DefaultConfig>,

    /// Target directory for this package (overrides CLI target)
    #[serde(default)]
    pub target_dir: Option<String>,

    /// Default build tags for this package
    #[serde(default)]
    pub build_tags: Option<Vec<String>>,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            include_all: false,
            default_tag: "default".to_string(),
        }
    }
}

/// Configuration for towboat deployment
#[derive(Debug)]
pub struct Config {
    /// Source directory containing dotfiles (package directory)
    pub package: PathBuf,

    /// Target directory where files will be deployed
    pub target_dir: PathBuf,

    /// Build tag to match for deployment (e.g., "linux", "macos", "windows")
    pub build_tag: String,

    /// Whether to run in dry-run mode (show what would be done without making changes)
    pub dry_run: bool,

    /// Whether to overwrite existing files in target directory
    pub force: bool,

    /// Whether to adopt existing files from target back to source
    pub adopt: bool,

    /// Whether to remove symlinks/files from target directory
    pub remove: bool,
}

/// Cache entry for a processed file
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheEntry {
    /// Source file path
    pub source_path: String,

    /// SHA256 hash of source file content
    pub source_hash: String,

    /// Deployed file path
    pub deployed_path: String,

    /// SHA256 hash of the processed content that was deployed
    pub deployed_hash: String,

    /// Build tag used when processing
    pub build_tag: String,
}

/// Cache file structure
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Cache {
    /// Map of target path -> cache entry
    #[serde(flatten)]
    pub entries: HashMap<String, CacheEntry>,
}

/// Process file content by extracting sections matching the build tag
///
/// This function processes file content containing build tag sections in the format:
/// ```text
/// # {tag-
/// content for tag
/// # -tag}
/// ```
///
/// # Arguments
///
/// * `content` - The file content to process
/// * `build_tag` - The build tag to match (e.g., "linux", "macos")
///
/// # Returns
///
/// Returns the processed content with matching tag sections extracted and
/// non-matching tag sections removed.
///
/// # Examples
///
/// ```rust
/// use towboat::process_file_with_build_tags;
///
/// let content = r#"# Common content
/// export PATH=$PATH:/usr/local/bin
///
/// # {linux-
/// alias ls='ls --color=auto'
/// # -linux}
///
/// # {macos-
/// alias ls='ls -G'
/// # -macos}
/// "#;
///
/// let result = process_file_with_build_tags(content, "linux").unwrap();
/// assert!(result.contains("--color=auto"));
/// ```
pub fn process_file_with_build_tags(content: &str, build_tag: &str) -> Result<String> {
    let escaped_tag = regex::escape(build_tag);
    let tag_pattern = format!(
        r"(?s)# \{{{}-\s*\n(.*?)\n# -{}\}}",
        escaped_tag, escaped_tag
    );
    let tag_regex = Regex::new(&tag_pattern)?;

    let mut result = content.to_string();

    // Replace build tag sections with their content
    result = tag_regex.replace_all(&result, "$1").to_string();

    // Remove other build tag sections
    let other_tags_regex = Regex::new(r"(?s)# \{[^}]+-\s*\n.*?\n# -[^}]+\}")?;
    result = other_tags_regex.replace_all(&result, "").to_string();

    Ok(result)
}

/// Compute SHA256 hash of content
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Get the cache file path based on the stow directory
fn get_cache_path(stow_dir: &Path) -> Result<PathBuf> {
    let cache_dir = stow_dir.join(".towboat");
    fs::create_dir_all(&cache_dir).context(format!(
        "Failed to create cache directory: {}",
        cache_dir.display()
    ))?;

    Ok(cache_dir.join("checksums.toml"))
}

/// Load cache from disk
pub fn load_cache(stow_dir: &Path) -> Result<Cache> {
    let cache_path = get_cache_path(stow_dir)?;

    if !cache_path.exists() {
        return Ok(Cache::default());
    }

    let content = fs::read_to_string(&cache_path).context(format!(
        "Failed to read cache file: {}",
        cache_path.display()
    ))?;

    let cache: Cache = toml::from_str(&content).context(format!(
        "Failed to parse cache file: {}",
        cache_path.display()
    ))?;

    Ok(cache)
}

/// Save cache to disk
pub fn save_cache(cache: &Cache, stow_dir: &Path) -> Result<()> {
    let cache_path = get_cache_path(stow_dir)?;

    let content = toml::to_string_pretty(cache).context("Failed to serialize cache")?;

    fs::write(&cache_path, content).context(format!(
        "Failed to write cache file: {}",
        cache_path.display()
    ))?;

    Ok(())
}

/// Parse a boat.toml file and return the configuration
///
/// # Arguments
///
/// * `config_path` - Path to the boat.toml file
///
/// # Returns
///
/// Returns the parsed BoatConfig or an error if parsing fails
pub fn parse_boat_config(config_path: &Path) -> Result<BoatConfig> {
    let content = fs::read_to_string(config_path).context(format!(
        "Failed to read boat.toml file: {}",
        config_path.display()
    ))?;

    let config: BoatConfig = toml::from_str(&content).context(format!(
        "Failed to parse boat.toml file: {}",
        config_path.display()
    ))?;

    Ok(config)
}

/// Find the applicable boat.toml file for a given directory
///
/// Searches upward from the given directory to find the nearest boat.toml file
///
/// # Arguments
///
/// * `dir` - Directory to start searching from
///
/// # Returns
///
/// Returns the path to the boat.toml file if found, None otherwise
pub fn find_boat_config(dir: &Path) -> Option<PathBuf> {
    let mut current = dir;
    loop {
        let config_path = current.join("boat.toml");
        if config_path.exists() && config_path.is_file() {
            return Some(config_path);
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    None
}

/// Check if a target should be included based on boat.toml configuration
///
/// # Arguments
///
/// * `target_path` - Path to the target (file or directory) to check
/// * `source_dir` - Source directory root
/// * `build_tag` - The build tag to match against
/// * `boat_config` - The boat.toml configuration
///
/// # Returns
///
/// Returns (should_include, target_path) where target_path is relative to target_dir
pub fn should_include_target_with_boat_config(
    target_path: &Path,
    source_dir: &Path,
    build_tag: &str,
    boat_config: &BoatConfig,
) -> Result<(bool, PathBuf)> {
    let relative_path = target_path
        .strip_prefix(source_dir)
        .context("Failed to get relative path")?;

    let path_str = relative_path.to_string_lossy().to_string();

    // Check if target is explicitly configured
    if let Some(target_config) = boat_config.targets.get(&path_str) {
        let should_include = target_config.tags.contains(&build_tag.to_string());
        // Use target if specified, otherwise default to source path
        let final_target = target_config
            .target
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| relative_path.to_path_buf());
        return Ok((should_include, final_target));
    }

    // Check if any parent directory is configured (for directory tag inheritance)
    // Walk up the path components to find a parent directory config
    let mut check_path = relative_path;
    while let Some(parent) = check_path.parent() {
        if parent == Path::new("") {
            break;
        }
        let parent_str = parent.to_string_lossy().to_string();
        if let Some(parent_config) = boat_config.targets.get(&parent_str) {
            let should_include = parent_config.tags.contains(&build_tag.to_string());
            // Inherit parent's tags, use original relative path as target
            return Ok((should_include, relative_path.to_path_buf()));
        }
        check_path = parent;
    }

    // Check default behavior
    let default_fallback = DefaultConfig::default();
    let default_config = boat_config.default.as_ref().unwrap_or(&default_fallback);

    // Check if file has build tag content (only for text files)
    if target_path.is_file() {
        // Try to read as UTF-8, skip if not valid text
        if let Ok(content) = fs::read_to_string(target_path) {
            let escaped_tag = regex::escape(build_tag);
            let tag_pattern = format!(r"# \{{{}-", escaped_tag);
            let tag_regex = Regex::new(&tag_pattern)?;
            if tag_regex.is_match(&content) {
                return Ok((true, relative_path.to_path_buf()));
            }
        }
    }

    if default_config.include_all {
        // If include_all is true, check if current build tag matches default_tag
        let should_include = build_tag == default_config.default_tag;
        return Ok((should_include, relative_path.to_path_buf()));
    }

    Ok((false, relative_path.to_path_buf()))
}

/// Discover all files in the source directory that match the build tag using boat.toml
///
/// Recursively walks the source directory to find files that should be included
/// based on boat.toml configuration. Directories are recursed into, but only individual
/// files are symlinked (not entire directories). If a subdirectory contains its own boat.toml,
/// that takes precedence for that subdirectory.
///
/// # Arguments
///
/// * `source_dir` - The directory to search for files
/// * `build_tag` - The build tag to match against
///
/// # Returns
///
/// Returns a vector of (source_path, target_path) tuples for files that match the build tag
pub fn discover_files_with_boat_config(
    source_dir: &Path,
    build_tag: &str,
) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut matching_targets = Vec::new();

    // Look for boat.toml file in source directory
    let config_path = match find_boat_config(source_dir) {
        Some(path) => path,
        None => {
            return Err(anyhow::anyhow!(
                "No boat.toml found in package directory: {}",
                source_dir.display()
            ));
        }
    };

    let boat_config = parse_boat_config(&config_path)?;

    for entry in WalkDir::new(source_dir).follow_links(false) {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        // Skip boat.toml files themselves
        if path.file_name() == Some(std::ffi::OsStr::new("boat.toml")) {
            continue;
        }

        // If this is a directory with its own boat.toml, let that handle its contents
        if path.is_dir() && path != source_dir {
            let nested_config = path.join("boat.toml");
            if nested_config.exists() {
                // Process this directory with its own config
                let nested_results = discover_files_with_boat_config(path, build_tag)?;
                matching_targets.extend(nested_results);
                // Skip traversing into this directory since we handled it
                continue;
            }
        }

        // Only check files - directories are recursed into but not symlinked
        if path.is_file() {
            // Skip broken symlinks in the source directory
            if path.is_symlink() && !path.exists() {
                eprintln!("Warning: Skipping broken symlink: {}", path.display());
                continue;
            }

            let (should_include, target_path) =
                should_include_target_with_boat_config(path, source_dir, build_tag, &boat_config)?;

            if should_include {
                matching_targets.push((path.to_path_buf(), target_path));
            }
        }
    }

    Ok(matching_targets)
}

/// Create a symlink or processed file at the target location
///
/// If the source file contains build tags, processes the content and writes a new file.
/// Otherwise, creates a symlink to the source file.
///
/// # Arguments
///
/// * `source` - Path to the source file
/// * `target` - Path where the file symlink should be created
/// * `build_tag` - The build tag for content processing
/// * `dry_run` - If true, only shows what would be done without making changes
/// * `force` - If true, overwrites existing files
/// * `adopt` - If true, adopts existing files back to source
/// * `cache` - Mutable reference to cache for tracking processed files
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the operation fails
pub fn create_symlink_or_file(
    source: &Path,
    target: &Path,
    build_tag: &str,
    dry_run: bool,
    force: bool,
    adopt: bool,
    cache: &mut Cache,
) -> Result<()> {
    // Verify source exists before doing anything
    if !source.exists() {
        return Err(anyhow::anyhow!(
            "Source file does not exist: {}. This may be a broken symlink or a file that was removed.",
            source.display()
        ));
    }

    // Check if target or any ancestor directory of target is a symlink pointing to source
    // This handles the case where a parent directory was previously symlinked
    let source_canon = source.canonicalize().ok();
    let mut check_parent = target.parent();
    while let Some(parent_dir) = check_parent {
        if parent_dir.is_symlink() {
            if let Ok(link_target) = fs::read_link(parent_dir) {
                let link_canon = if link_target.is_absolute() {
                    link_target.canonicalize().ok()
                } else {
                    parent_dir
                        .parent()
                        .and_then(|p| p.join(&link_target).canonicalize().ok())
                };

                // Check if the symlink points to a directory containing our source
                if let (Some(src), Some(lnk)) = (source_canon.as_ref(), link_canon.as_ref()) {
                    if src.starts_with(lnk) {
                        // An ancestor directory is symlinked to contain our source
                        // File is already correctly deployed via ancestor directory symlink
                        return Ok(());
                    }
                }
            }
        }
        check_parent = parent_dir.parent();
    }

    // Handle adopt mode - copy target to source
    if adopt && target.exists() {
        if dry_run {
            println!("Would adopt: {} <- {}", source.display(), target.display());
        } else {
            // Create parent directory if needed
            if let Some(parent) = source.parent()
                && !parent.exists()
            {
                fs::create_dir_all(parent)
                    .context(format!("Failed to create directory: {}", parent.display()))?;
            }

            fs::copy(target, source).context(format!(
                "Failed to adopt: {} <- {}",
                source.display(),
                target.display()
            ))?;
            println!("Adopted: {} <- {}", source.display(), target.display());
        }
        return Ok(());
    }

    // Create parent directory if needed
    if let Some(parent) = target.parent()
        && !parent.exists()
    {
        if dry_run {
            println!("Would create directory: {}", parent.display());
        } else {
            fs::create_dir_all(parent)
                .context(format!("Failed to create directory: {}", parent.display()))?;
        }
    }

    // Check if source file has build tags (to determine if we need cache checking)
    let source_has_build_tags = if let Ok(content) = fs::read_to_string(source) {
        let escaped_tag = regex::escape(build_tag);
        let tag_pattern = format!(r"# \{{{}-", escaped_tag);
        if let Ok(tag_regex) = Regex::new(&tag_pattern) {
            tag_regex.is_match(&content)
        } else {
            false
        }
    } else {
        false
    };

    // Handle existing targets
    if target.exists() {
        // Check if target is a symlink pointing to source already
        if target.is_symlink()
            && let Ok(link_target) = fs::read_link(target)
        {
            // Canonicalize both paths for comparison
            let canonical_source = source.canonicalize().ok();
            let canonical_link_target = if link_target.is_absolute() {
                link_target.canonicalize().ok()
            } else {
                // Relative symlink - resolve relative to target's parent directory
                target
                    .parent()
                    .and_then(|parent| parent.join(&link_target).canonicalize().ok())
            };

            if let (Some(src), Some(tgt)) = (canonical_source, canonical_link_target)
                && src == tgt
            {
                // Already correctly symlinked, nothing to do
                return Ok(());
            }
        }

        // For processed files (those with build tags), check cache before deciding what to do
        if source_has_build_tags && !target.is_symlink() {
            let target_key = target.to_string_lossy().to_string();
            if let Some(cache_entry) = cache.entries.get(&target_key) {
                // Read current target content
                if let Ok(target_content) = fs::read_to_string(target) {
                    let target_hash = compute_hash(&target_content);

                    // If target was modified by user (hash doesn't match cache)
                    if target_hash != cache_entry.deployed_hash {
                        if !force {
                            return Err(anyhow::anyhow!(
                                "Target file has been manually modified: {}\n\
                                The file was previously deployed by towboat but has local changes.\n\
                                Options:\n\
                                  --force  Overwrite with newly processed content (loses manual edits)\n\
                                  --adopt  Copy current target back to source package",
                                target.display()
                            ));
                        }
                        println!(
                            "Warning: Overwriting manually modified file: {}",
                            target.display()
                        );
                    }
                }
            }
        }

        if !force && !adopt {
            return Err(anyhow::anyhow!(
                "Target exists: {}. Use --force to overwrite or --adopt to adopt back to package.",
                target.display()
            ));
        }

        // Remove existing target if force is enabled
        if force {
            if dry_run {
                println!("Would remove existing: {}", target.display());
            } else if target.is_symlink() || target.is_file() {
                fs::remove_file(target)
                    .context(format!("Failed to remove existing: {}", target.display()))?;
            }
        }
    }

    // Check if this is a text file with build tags that need processing
    if let Ok(content) = fs::read_to_string(source) {
        let escaped_tag = regex::escape(build_tag);
        let tag_pattern = format!(r"# \{{{}-", escaped_tag);
        let tag_regex = Regex::new(&tag_pattern)?;

        if tag_regex.is_match(&content) {
            // File has build tags - needs processing
            let source_hash = compute_hash(&content);
            let processed_content = process_file_with_build_tags(&content, build_tag)?;
            let processed_hash = compute_hash(&processed_content);

            // Cache check already happened earlier, now just process and deploy
            if dry_run {
                println!(
                    "Would create processed file: {} -> {}",
                    source.display(),
                    target.display()
                );
            } else {
                fs::write(target, &processed_content).context(format!(
                    "Failed to write processed file: {}",
                    target.display()
                ))?;
                println!("Created processed file: {}", target.display());

                // Update cache
                let target_key = target.to_string_lossy().to_string();
                cache.entries.insert(
                    target_key,
                    CacheEntry {
                        source_path: source.to_string_lossy().to_string(),
                        source_hash,
                        deployed_path: target.to_string_lossy().to_string(),
                        deployed_hash: processed_hash,
                        build_tag: build_tag.to_string(),
                    },
                );
            }
            return Ok(());
        }
    }

    // No build tags - create symlink for file or binary file
    // Canonicalize the source path to ensure it's an absolute path
    let canonical_source = source.canonicalize().context(format!(
        "Failed to canonicalize source path: {}",
        source.display()
    ))?;

    if dry_run {
        println!(
            "Would create symlink: {} -> {}",
            canonical_source.display(),
            target.display()
        );
    } else {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&canonical_source, target).context(format!(
                "Failed to create symlink: {} -> {}",
                canonical_source.display(),
                target.display()
            ))?;
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(&canonical_source, target).context(format!(
                "Failed to create file symlink: {} -> {}",
                canonical_source.display(),
                target.display()
            ))?;
        }
        println!("Created symlink: {}", target.display());
    }

    Ok(())
}

/// Remove a symlink or file from the target directory
///
/// If removing a file leaves behind an empty directory, the directory is also removed.
///
/// # Arguments
///
/// * `target` - Path to the file/symlink to remove
/// * `dry_run` - If true, only shows what would be done without making changes
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the operation fails
pub fn remove_symlink_or_file(target: &Path, dry_run: bool) -> Result<()> {
    if !target.exists() {
        // File doesn't exist, nothing to do
        return Ok(());
    }

    if dry_run {
        println!("Would remove: {}", target.display());
    } else if target.is_symlink() || target.is_file() {
        fs::remove_file(target).context(format!("Failed to remove file: {}", target.display()))?;
        println!("Removed: {}", target.display());
    } else if target.is_dir() {
        fs::remove_dir_all(target)
            .context(format!("Failed to remove directory: {}", target.display()))?;
        println!("Removed directory: {}", target.display());
    }

    // Remove empty parent directories
    if let Some(mut parent) = target.parent() {
        while parent.exists() {
            // Check if directory is empty
            match fs::read_dir(parent) {
                Ok(mut entries) => {
                    if entries.next().is_none() {
                        // Directory is empty, remove it
                        if dry_run {
                            println!("Would remove empty directory: {}", parent.display());
                        } else {
                            fs::remove_dir(parent).context(format!(
                                "Failed to remove empty directory: {}",
                                parent.display()
                            ))?;
                            println!("Removed empty directory: {}", parent.display());
                        }
                        // Move up to parent
                        if let Some(next_parent) = parent.parent() {
                            parent = next_parent;
                        } else {
                            break;
                        }
                    } else {
                        // Directory not empty, stop
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    Ok(())
}

/// Main entry point for towboat deployment
///
/// Executes the complete towboat workflow: discovers files, processes them according
/// to build tags, and deploys them to the target directory.
///
/// # Arguments
///
/// * `config` - Configuration containing source/target directories, build tag, and options
///
/// # Returns
///
/// Returns `Ok(())` on successful deployment, or an error if any step fails
///
/// # Examples
///
/// ```rust
/// use towboat::{Config, run_towboat};
/// use std::path::PathBuf;
///
/// let config = Config {
///     source_dir: PathBuf::from("./dotfiles/home"),
///     stow_dir: PathBuf::from("./dotfiles"),
///     target_dir: PathBuf::from("/home/user"),
///     build_tag: "linux".to_string(),
///     dry_run: true, // Preview mode
///     force: false,
///     adopt: false,
///     remove: false,
/// };
///
/// // This would show what files would be deployed
/// // run_towboat(config).unwrap();
/// ```
pub fn run_towboat(config: Config) -> Result<()> {
    if !config.package.exists() {
        return Err(anyhow::anyhow!(
            "Source directory does not exist: {}",
            config.package.display()
        ));
    }

    let target_dir = if config.target_dir.is_relative() {
        std::env::current_dir()?.join(&config.target_dir)
    } else {
        config.target_dir.clone()
    };

    // Load cache (only needed if not in remove mode)
    let mut cache = if !config.remove {
        load_cache(&config.package)?
    } else {
        Cache::default()
    };

    println!("Towboat - Cross-platform dotfile manager");
    println!("Source: {}", config.package.display());
    println!("Target: {}", target_dir.display());
    println!("Build tag: {}", config.build_tag);
    if config.dry_run {
        println!("DRY RUN - No changes will be made");
    }
    println!();

    let matching_files = discover_files_with_boat_config(&config.package, &config.build_tag)?;

    if matching_files.is_empty() {
        println!("No files found matching build tag '{}'", config.build_tag);
        return Ok(());
    }

    println!("Found {} matching files:", matching_files.len());

    if config.remove {
        // Remove mode - remove files from target directory
        for (source_file, target_relative_path) in &matching_files {
            let target_path = target_dir.join(target_relative_path);

            println!(
                "Processing: {} (removing from {})",
                source_file.display(),
                target_path.display()
            );

            remove_symlink_or_file(&target_path, config.dry_run)?;
        }

        if config.dry_run {
            println!("\nDry run completed. Use without --dry-run to apply changes.");
        } else {
            println!("\nRemoval completed successfully!");
        }
    } else {
        // Normal mode - create symlinks/files
        for (source_file, target_relative_path) in &matching_files {
            let target_path = target_dir.join(target_relative_path);

            println!(
                "Processing: {} -> {}",
                source_file.display(),
                target_path.display()
            );

            create_symlink_or_file(
                source_file,
                &target_path,
                &config.build_tag,
                config.dry_run,
                config.force,
                config.adopt,
                &mut cache,
            )?;
        }

        // Save cache after successful deployment (not in dry-run mode)
        if !config.dry_run {
            save_cache(&cache, &config.package)?;
        }

        if config.dry_run {
            println!("\nDry run completed. Use without --dry-run to apply changes.");
        } else {
            println!("\nCompleted successfully!");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_boat_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("boat.toml");

        let config_content = r#"
target_dir = "~/.config"
build_tags = ["linux", "macos"]

[targets]
".bashrc" = { target = ".bashrc", tags = ["linux", "macos"] }
".vimrc" = { target = ".vimrc", tags = ["linux"] }
"scripts" = { tags = ["linux"] }

[default]
include_all = false
"#;

        fs::write(&config_path, config_content).unwrap();

        let config = parse_boat_config(&config_path).unwrap();

        assert_eq!(config.targets.len(), 3);
        assert!(config.targets.contains_key(".bashrc"));
        assert!(config.targets.contains_key(".vimrc"));
        assert!(config.targets.contains_key("scripts"));

        let bashrc_config = &config.targets[".bashrc"];
        assert_eq!(bashrc_config.target, Some(".bashrc".to_string()));
        assert_eq!(bashrc_config.tags, vec!["linux", "macos"]);

        let default_config = config.default.unwrap();
        assert!(!default_config.include_all);

        assert_eq!(config.target_dir, Some("~/.config".to_string()));
        assert_eq!(
            config.build_tags,
            Some(vec!["linux".to_string(), "macos".to_string()])
        );
    }

    #[test]
    fn test_should_include_target_with_boat_config() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path();
        let file_path = source_dir.join(".bashrc");
        fs::write(&file_path, "content").unwrap();

        let boat_config = BoatConfig {
            targets: {
                let mut targets = HashMap::new();
                targets.insert(
                    ".bashrc".to_string(),
                    TargetConfig {
                        target: Some(".bashrc".to_string()),
                        tags: vec!["linux".to_string(), "macos".to_string()],
                    },
                );
                targets
            },
            default: Some(DefaultConfig {
                include_all: false,
                default_tag: "default".to_string(),
            }),
            target_dir: None,
            build_tags: None,
        };

        let (should_include, target_path) =
            should_include_target_with_boat_config(&file_path, source_dir, "linux", &boat_config)
                .unwrap();

        assert!(should_include);
        assert_eq!(target_path, PathBuf::from(".bashrc"));

        let (should_include, _) =
            should_include_target_with_boat_config(&file_path, source_dir, "windows", &boat_config)
                .unwrap();

        assert!(!should_include);
    }

    #[test]
    fn test_discover_files_with_boat_config() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path();

        // Create boat.toml file
        let config_content = r#"
[targets]
".bashrc" = { target = ".bashrc", tags = ["linux"] }
".vimrc" = { target = ".vimrc", tags = ["macos"] }

[default]
include_all = false
"#;
        fs::write(source_dir.join("boat.toml"), config_content).unwrap();

        // Create test files
        fs::write(source_dir.join(".bashrc"), "linux bash content").unwrap();
        fs::write(source_dir.join(".vimrc"), "macos vim content").unwrap();
        fs::write(source_dir.join("README.md"), "readme content").unwrap();

        let files = discover_files_with_boat_config(source_dir, "linux").unwrap();

        assert_eq!(files.len(), 1);
        let (source_path, target_path) = &files[0];
        assert!(source_path.file_name().unwrap() == ".bashrc");
        assert_eq!(target_path, &PathBuf::from(".bashrc"));
    }

    #[test]
    fn test_process_file_with_build_tags_linux() {
        let content = r#"# Common content
export PATH=$PATH:/usr/local/bin

# {linux-
alias ls='ls --color=auto'
export EDITOR=vim
# -linux}

# {macos-
alias ls='ls -G'
export EDITOR=nano
# -macos}

# More common content
echo "Hello from shell""#;

        let result = process_file_with_build_tags(content, "linux").unwrap();

        assert!(result.contains("alias ls='ls --color=auto'"));
        assert!(result.contains("export EDITOR=vim"));
        assert!(!result.contains("alias ls='ls -G'"));
        assert!(!result.contains("export EDITOR=nano"));
        assert!(result.contains("# Common content"));
        assert!(result.contains("echo \"Hello from shell\""));
    }

    #[test]
    fn test_process_file_with_build_tags_macos() {
        let content = r#"# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}

# {windows-
alias ls='dir'
# -windows}"#;

        let result = process_file_with_build_tags(content, "macos").unwrap();

        assert!(result.contains("alias ls='ls -G'"));
        assert!(!result.contains("alias ls='ls --color=auto'"));
        assert!(!result.contains("alias ls='dir'"));
    }

    #[test]
    fn test_process_file_with_toml_style_tags() {
        // Test TOML files where commented lines are part of the build tag
        let content = r#"[font]
# {linux-
# size = 10.0
# -linux}
# {macos-
size = 16.0
# -macos}
"#;

        let result_macos = process_file_with_build_tags(content, "macos").unwrap();
        assert!(
            result_macos.contains("size = 16.0"),
            "Expected 'size = 16.0' in macos result, got:\n{}",
            result_macos
        );
        assert!(
            !result_macos.contains("# size = 10.0"),
            "Should not contain linux commented line"
        );

        let result_linux = process_file_with_build_tags(content, "linux").unwrap();
        assert!(
            result_linux.contains("# size = 10.0"),
            "Expected '# size = 10.0' in linux result, got:\n{}",
            result_linux
        );
        assert!(
            !result_linux.contains("size = 16.0"),
            "Should not contain macos line"
        );
    }

    #[test]
    fn test_cache_detects_modified_file() {
        let temp_dir = TempDir::new().unwrap();
        let source_file = temp_dir.path().join("source.sh");
        let target_file = temp_dir.path().join("target.sh");

        // Create source file with build tags
        let source_content = r#"# Common content
# {linux-
export LINUX_VAR=1
# -linux}
"#;
        fs::write(&source_file, source_content).unwrap();

        // Create initial cache and deploy file
        let mut cache = Cache::default();
        create_symlink_or_file(
            &source_file,
            &target_file,
            "linux",
            false,
            false,
            false,
            &mut cache,
        )
        .unwrap();

        // Verify file was created
        assert!(target_file.exists());
        let deployed_content = fs::read_to_string(&target_file).unwrap();
        assert!(deployed_content.contains("export LINUX_VAR=1"));

        // Verify cache entry exists
        let target_key = target_file.to_string_lossy().to_string();
        assert!(cache.entries.contains_key(&target_key));

        // Manually modify the target file
        let modified_content = "# Modified by user\nexport USER_VAR=2\n";
        fs::write(&target_file, modified_content).unwrap();

        // Try to deploy again without --force - should fail
        let result = create_symlink_or_file(
            &source_file,
            &target_file,
            "linux",
            false,
            false,
            false,
            &mut cache,
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("manually modified")
        );

        // Deploy again with --force - should succeed
        let result = create_symlink_or_file(
            &source_file,
            &target_file,
            "linux",
            false,
            true,
            false,
            &mut cache,
        );
        assert!(result.is_ok());

        // Verify file was overwritten
        let final_content = fs::read_to_string(&target_file).unwrap();
        assert!(final_content.contains("export LINUX_VAR=1"));
        assert!(!final_content.contains("USER_VAR"));
    }

    #[test]
    fn test_compute_hash() {
        let content1 = "hello world";
        let content2 = "hello world";
        let content3 = "different content";

        let hash1 = compute_hash(content1);
        let hash2 = compute_hash(content2);
        let hash3 = compute_hash(content3);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        // Different content should produce different hash
        assert_ne!(hash1, hash3);
        // Hash should be 64 hex characters (SHA256)
        assert_eq!(hash1.len(), 64);
    }
}
