//! Towboat v2 — A cross-platform dotfile manager with build tags and templates.
//!
//! All files are resolved (tag processing + template substitution) into
//! `.towboat/resolved/`, and symlinks always point to resolved files.

pub mod commands;
pub mod config;
pub mod deploy;
pub mod discovery;
pub mod error;
pub mod resolve;
pub mod tags;
pub mod template;

// Re-exports for convenience
pub use config::manifest::SystemManifest;
pub use config::package::PackageConfig;
pub use deploy::lock::{FileState, LockEntry, LockFile};
pub use deploy::symlink;
pub use discovery::walker::DiscoveredFile;
pub use error::{Result, TowboatError};
pub use resolve::resolver::{compute_hash, resolve_file, resolve_package};
pub use resolve::{ResolveOutcome, ResolvedFile};
