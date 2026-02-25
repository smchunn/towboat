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
