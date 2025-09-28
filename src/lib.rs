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
//!     source_dir: PathBuf::from("./dotfiles"),
//!     target_dir: PathBuf::from("/home/user"),
//!     build_tag: "linux".to_string(),
//!     dry_run: false,
//! };
//!
//! // This would deploy Linux-specific dotfiles
//! // run_towboat(config).unwrap();
//! ```

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// File configuration from .boatrc
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileConfig {
    /// Target path for the file (relative to target directory)
    pub target: String,
    /// Build tags this file should be included for
    pub tags: Vec<String>,
}

/// Directory configuration from .boatrc
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DirectoryConfig {
    /// Build tags this directory should be included for
    pub tags: Vec<String>,
}

/// Default configuration behavior
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultConfig {
    /// Whether to include all files/directories not explicitly configured
    pub include_all: bool,
}

/// .boatrc configuration file structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoatConfig {
    /// File-specific configurations
    #[serde(default)]
    pub files: HashMap<String, FileConfig>,
    /// Directory-specific configurations
    #[serde(default)]
    pub directories: HashMap<String, DirectoryConfig>,
    /// Default behavior configuration
    #[serde(default)]
    pub default: Option<DefaultConfig>,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self { include_all: false }
    }
}

/// Configuration for towboat deployment
#[derive(Debug)]
pub struct Config {
    /// Source directory containing dotfiles
    pub source_dir: PathBuf,
    /// Target directory where files will be deployed
    pub target_dir: PathBuf,
    /// Build tag to match for deployment (e.g., "linux", "macos", "windows")
    pub build_tag: String,
    /// Whether to run in dry-run mode (show what would be done without making changes)
    pub dry_run: bool,
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
/// assert!(!result.contains("ls -G"));
/// ```
pub fn process_file_with_build_tags(content: &str, build_tag: &str) -> Result<String> {
    let escaped_tag = regex::escape(build_tag);
    let tag_pattern = format!(r"(?s)# \{{{}-\s*\n(.*?)\n# -{}\}}", escaped_tag, escaped_tag);
    let tag_regex = Regex::new(&tag_pattern)?;

    let mut result = content.to_string();

    // Replace build tag sections with their content
    result = tag_regex.replace_all(&result, "$1").to_string();

    // Remove other build tag sections
    let other_tags_regex = Regex::new(r"(?s)# \{[^}]+-\s*\n.*?\n# -[^}]+\}")?;
    result = other_tags_regex.replace_all(&result, "").to_string();

    Ok(result)
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
    let content = fs::read_to_string(config_path)
        .context(format!("Failed to read boat.toml file: {}", config_path.display()))?;

    let config: BoatConfig = toml::from_str(&content)
        .context(format!("Failed to parse boat.toml file: {}", config_path.display()))?;

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

/// Check if a file should be included based on boat.toml configuration
///
/// # Arguments
///
/// * `file_path` - Path to the file to check
/// * `source_dir` - Source directory root
/// * `build_tag` - The build tag to match against
/// * `boat_config` - The boat.toml configuration
///
/// # Returns
///
/// Returns (should_include, target_path) where target_path is relative to target_dir
pub fn should_include_file_with_boat_config(
    file_path: &Path,
    source_dir: &Path,
    build_tag: &str,
    boat_config: &BoatConfig,
) -> Result<(bool, PathBuf)> {
    let relative_path = file_path.strip_prefix(source_dir)
        .context("Failed to get relative path")?;

    let filename = relative_path.to_string_lossy().to_string();

    // Check if file is explicitly configured
    if let Some(file_config) = boat_config.files.get(&filename) {
        let should_include = file_config.tags.contains(&build_tag.to_string());
        let target_path = PathBuf::from(&file_config.target);
        return Ok((should_include, target_path));
    }

    // Check if file's parent directory is configured
    if let Some(parent) = relative_path.parent() {
        let parent_str = parent.to_string_lossy().to_string();
        if let Some(dir_config) = boat_config.directories.get(&parent_str) {
            let should_include = dir_config.tags.contains(&build_tag.to_string());
            return Ok((should_include, relative_path.to_path_buf()));
        }
    }

    // Check default behavior
    let default_fallback = DefaultConfig::default();
    let default_config = boat_config.default.as_ref().unwrap_or(&default_fallback);
    if default_config.include_all {
        // Still need to check for build tag content in the file
        if file_path.is_file() {
            let content = fs::read_to_string(file_path)
                .context(format!("Failed to read file: {}", file_path.display()))?;

            let escaped_tag = regex::escape(build_tag);
            let tag_pattern = format!(r"# \{{{}-", escaped_tag);
            let tag_regex = Regex::new(&tag_pattern)?;
            if tag_regex.is_match(&content) {
                return Ok((true, relative_path.to_path_buf()));
            }
        }

        // Include by default if include_all is true and no build tags found
        return Ok((true, relative_path.to_path_buf()));
    }

    Ok((false, relative_path.to_path_buf()))
}

/// Determine if a file should be included based on the build tag
///
/// A file is included if:
/// 1. Its filename contains the build tag (e.g., `.bashrc.linux` for "linux" tag)
/// 2. Its content contains build tag sections (e.g., `# {linux-...# -linux}`)
///
/// # Arguments
///
/// * `file_path` - Path to the file to check
/// * `build_tag` - The build tag to match against
///
/// # Returns
///
/// Returns `true` if the file should be included for this build tag
pub fn should_include_file(file_path: &Path, build_tag: &str) -> Result<bool> {
    // Check if filename contains build tag
    if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
        if filename.contains(&format!(".{}", build_tag)) {
            return Ok(true);
        }
    }

    // Check if file contains build tags in content
    if file_path.is_file() {
        let content = fs::read_to_string(file_path)
            .context(format!("Failed to read file: {}", file_path.display()))?;

        let escaped_tag = regex::escape(build_tag);
        let tag_pattern = format!(r"# \{{{}-", escaped_tag);
        let tag_regex = Regex::new(&tag_pattern)?;
        if tag_regex.is_match(&content) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Discover all files in the source directory that match the build tag using boat.toml
///
/// Recursively walks the source directory to find files that should be included
/// based on boat.toml configuration.
///
/// # Arguments
///
/// * `source_dir` - The directory to search for files
/// * `build_tag` - The build tag to match against
///
/// # Returns
///
/// Returns a vector of (source_path, target_path) tuples for files that match the build tag
pub fn discover_files_with_boat_config(source_dir: &Path, build_tag: &str) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut matching_files = Vec::new();

    // Look for boat.toml file in source directory
    let config_path = match find_boat_config(source_dir) {
        Some(path) => path,
        None => {
            // Fall back to legacy behavior if no boat.toml found
            let legacy_files = discover_files(source_dir, build_tag)?;
            return Ok(legacy_files.into_iter().map(|p| {
                let relative_path = p.strip_prefix(source_dir).unwrap_or(&p);

                // Remove build tag from filename if present (legacy behavior)
                let target_filename = if let Some(filename) = relative_path.file_name().and_then(|n| n.to_str()) {
                    let clean_filename = filename.replace(&format!(".{}", build_tag), "");
                    if let Some(parent) = relative_path.parent() {
                        parent.join(clean_filename)
                    } else {
                        PathBuf::from(clean_filename)
                    }
                } else {
                    relative_path.to_path_buf()
                };

                (p.clone(), target_filename)
            }).collect());
        }
    };

    let boat_config = parse_boat_config(&config_path)?;

    for entry in WalkDir::new(source_dir) {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        // Skip boat.toml files themselves
        if path.file_name() == Some(std::ffi::OsStr::new("boat.toml")) {
            continue;
        }

        if path.is_file() {
            let (should_include, target_path) = should_include_file_with_boat_config(
                path, source_dir, build_tag, &boat_config
            )?;

            if should_include {
                matching_files.push((path.to_path_buf(), target_path));
            }
        }
    }

    Ok(matching_files)
}

/// Discover all files in the source directory that match the build tag
///
/// Recursively walks the source directory to find files that should be included
/// based on the build tag criteria.
///
/// # Arguments
///
/// * `source_dir` - The directory to search for files
/// * `build_tag` - The build tag to match against
///
/// # Returns
///
/// Returns a vector of file paths that match the build tag
pub fn discover_files(source_dir: &Path, build_tag: &str) -> Result<Vec<PathBuf>> {
    let mut matching_files = Vec::new();

    for entry in WalkDir::new(source_dir) {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.is_file() && should_include_file(path, build_tag)? {
            matching_files.push(path.to_path_buf());
        }
    }

    Ok(matching_files)
}

/// Create a symlink or processed file at the target location
///
/// If the source file contains build tags, processes the content and writes a new file.
/// Otherwise, creates a symlink to the source file.
///
/// # Arguments
///
/// * `source` - Path to the source file
/// * `target` - Path where the file/symlink should be created
/// * `build_tag` - The build tag for content processing
/// * `dry_run` - If true, only shows what would be done without making changes
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if the operation fails
pub fn create_symlink_or_file(source: &Path, target: &Path, build_tag: &str, dry_run: bool) -> Result<()> {
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            if dry_run {
                println!("Would create directory: {}", parent.display());
            } else {
                fs::create_dir_all(parent)
                    .context(format!("Failed to create directory: {}", parent.display()))?;
            }
        }
    }

    // Check if this file has build tags that need processing
    let content = fs::read_to_string(source)
        .context(format!("Failed to read source file: {}", source.display()))?;

    let escaped_tag = regex::escape(build_tag);
    let tag_pattern = format!(r"# \{{{}-", escaped_tag);
    let tag_regex = Regex::new(&tag_pattern)?;

    if tag_regex.is_match(&content) {
        // Process the file content and write it instead of symlinking
        let processed_content = process_file_with_build_tags(&content, build_tag)?;

        if dry_run {
            println!("Would create processed file: {} -> {}", source.display(), target.display());
        } else {
            fs::write(target, processed_content)
                .context(format!("Failed to write processed file: {}", target.display()))?;
            println!("Created processed file: {}", target.display());
        }
    } else {
        // Create symlink for files without build tags
        if dry_run {
            println!("Would create symlink: {} -> {}", source.display(), target.display());
        } else {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(source, target)
                    .context(format!("Failed to create symlink: {} -> {}", source.display(), target.display()))?;
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_file(source, target)
                    .context(format!("Failed to create symlink: {} -> {}", source.display(), target.display()))?;
            }
            println!("Created symlink: {}", target.display());
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
///     source_dir: PathBuf::from("./dotfiles"),
///     target_dir: PathBuf::from("/home/user"),
///     build_tag: "linux".to_string(),
///     dry_run: true, // Preview mode
/// };
///
/// // This would show what files would be deployed
/// // run_towboat(config).unwrap();
/// ```
pub fn run_towboat(config: Config) -> Result<()> {
    if !config.source_dir.exists() {
        return Err(anyhow::anyhow!("Source directory does not exist: {}", config.source_dir.display()));
    }

    let target_dir = if config.target_dir.is_relative() {
        std::env::current_dir()?.join(&config.target_dir)
    } else {
        config.target_dir.clone()
    };

    println!("Towboat - Cross-platform dotfile manager");
    println!("Source: {}", config.source_dir.display());
    println!("Target: {}", target_dir.display());
    println!("Build tag: {}", config.build_tag);
    if config.dry_run {
        println!("DRY RUN - No changes will be made");
    }
    println!();

    let matching_files = discover_files_with_boat_config(&config.source_dir, &config.build_tag)?;

    if matching_files.is_empty() {
        println!("No files found matching build tag '{}'", config.build_tag);
        return Ok(());
    }

    println!("Found {} matching files:", matching_files.len());

    for (source_file, target_relative_path) in &matching_files {
        let target_path = target_dir.join(target_relative_path);

        println!("Processing: {} -> {}", source_file.display(), target_path.display());

        create_symlink_or_file(source_file, &target_path, &config.build_tag, config.dry_run)?;
    }

    if config.dry_run {
        println!("\nDry run completed. Use without --dry-run to apply changes.");
    } else {
        println!("\nCompleted successfully!");
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
[files]
".bashrc" = { target = ".bashrc", tags = ["linux", "macos"] }
".vimrc" = { target = ".vimrc", tags = ["linux"] }

[directories]
"scripts" = { tags = ["linux"] }

[default]
include_all = false
"#;

        fs::write(&config_path, config_content).unwrap();

        let config = parse_boat_config(&config_path).unwrap();

        assert_eq!(config.files.len(), 2);
        assert!(config.files.contains_key(".bashrc"));
        assert!(config.files.contains_key(".vimrc"));

        let bashrc_config = &config.files[".bashrc"];
        assert_eq!(bashrc_config.target, ".bashrc");
        assert_eq!(bashrc_config.tags, vec!["linux", "macos"]);

        assert_eq!(config.directories.len(), 1);
        assert!(config.directories.contains_key("scripts"));

        let default_config = config.default.unwrap();
        assert!(!default_config.include_all);
    }

    #[test]
    fn test_should_include_file_with_boat_config() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path();
        let file_path = source_dir.join(".bashrc");
        fs::write(&file_path, "content").unwrap();

        let boat_config = BoatConfig {
            files: {
                let mut files = HashMap::new();
                files.insert(".bashrc".to_string(), FileConfig {
                    target: ".bashrc".to_string(),
                    tags: vec!["linux".to_string(), "macos".to_string()],
                });
                files
            },
            directories: HashMap::new(),
            default: Some(DefaultConfig { include_all: false }),
        };

        let (should_include, target_path) = should_include_file_with_boat_config(
            &file_path, source_dir, "linux", &boat_config
        ).unwrap();

        assert!(should_include);
        assert_eq!(target_path, PathBuf::from(".bashrc"));

        let (should_include, _) = should_include_file_with_boat_config(
            &file_path, source_dir, "windows", &boat_config
        ).unwrap();

        assert!(!should_include);
    }

    #[test]
    fn test_discover_files_with_boat_config() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path();

        // Create boat.toml file
        let config_content = r#"
[files]
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
    fn test_discover_files_with_boat_config_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path();

        // Create test files without boat.toml (should fall back to legacy behavior)
        fs::write(source_dir.join(".bashrc.linux"), "linux content").unwrap();
        fs::write(source_dir.join(".vimrc.macos"), "macos content").unwrap();

        let files = discover_files_with_boat_config(source_dir, "linux").unwrap();

        assert_eq!(files.len(), 1);
        let (source_path, target_path) = &files[0];
        assert!(source_path.file_name().unwrap() == ".bashrc.linux");
        // In legacy mode, build tag should be removed from target filename
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
    fn test_should_include_file_by_filename() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join(".bashrc.linux");
        fs::write(&file_path, "content").unwrap();

        let result = should_include_file(&file_path, "linux").unwrap();
        assert!(result);

        let result = should_include_file(&file_path, "macos").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_discover_files() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path();

        // Create test files
        fs::write(source_dir.join(".bashrc.linux"), "linux content").unwrap();
        fs::write(source_dir.join(".vimrc.macos"), "macos content").unwrap();
        fs::write(source_dir.join(".gitconfig"), r#"# {linux-
linux git config
# -linux}"#).unwrap();
        fs::write(source_dir.join("README.md"), "common content").unwrap();

        let files = discover_files(source_dir, "linux").unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.file_name().unwrap() == ".bashrc.linux"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == ".gitconfig"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == ".vimrc.macos"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == "README.md"));
    }
}