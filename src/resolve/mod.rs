//! Resolution pipeline: source files → tag processing → template substitution → resolved output.

pub mod resolver;

use std::path::PathBuf;

/// A file that has been resolved (tags processed, templates substituted).
#[derive(Debug, Clone)]
pub struct ResolvedFile {
    /// Package name.
    pub package: String,
    /// Source file path relative to stow directory (e.g. "bash/.bashrc").
    pub source_relative: PathBuf,
    /// Resolved file content.
    pub content: String,
    /// Target path relative to target directory (e.g. ".bashrc").
    pub target_relative: PathBuf,
    /// Whether the source contained build tag sections.
    pub had_tags: bool,
    /// Tags that were matched when including this file.
    pub matched_expr: String,
}

/// Outcome of resolving a package.
#[derive(Debug, Default)]
pub struct ResolveOutcome {
    /// Successfully resolved files.
    pub resolved: Vec<ResolvedFile>,
    /// Errors that occurred during resolution (non-fatal, collected).
    pub errors: Vec<String>,
}
