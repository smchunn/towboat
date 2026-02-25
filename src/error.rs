use std::path::PathBuf;

/// Typed error variants for the towboat library.
///
/// Commands wrap these in `anyhow::Result` for context;
/// library callers can match on specific variants.
#[derive(Debug, thiserror::Error)]
pub enum TowboatError {
    #[error("manifest not found: expected towboat.toml at {0}")]
    ManifestNotFound(PathBuf),

    #[error("package config not found: expected boat.toml at {0}")]
    PackageConfigNotFound(PathBuf),

    #[error("invalid tag expression: {0}")]
    InvalidTagExpr(String),

    #[error("undefined variable: {{{{{name}}}}}")]
    UndefinedVariable { name: String },

    #[error("conflict on {path}: source and resolved file both changed since last sync")]
    Conflict { path: String },

    #[error("target already exists: {0} (use --force to overwrite)")]
    TargetExists(PathBuf),

    #[error("failed to create symlink: {link_source} -> {link_target}: {reason}")]
    SymlinkFailed {
        link_source: PathBuf,
        link_target: PathBuf,
        reason: String,
    },

    #[error("lock file corrupt: {0}")]
    LockCorrupt(String),

    #[error("package not found: {0}")]
    PackageNotFound(String),

    #[error("mismatched tag delimiters: opened with {open:?} but closed with {close:?}")]
    MismatchedTagDelimiters { open: String, close: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TowboatError>;
