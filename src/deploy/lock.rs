//! Lock file (`towboat.lock`) management.
//!
//! The lock file tracks the state of all deployed files:
//! - Source hash and resolved hash enable three-way drift detection
//! - Tags matched at deployment time
//! - Symlink target path

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TowboatError};

/// The lock file structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LockFile {
    /// Lock file format version.
    #[serde(default = "default_version")]
    pub version: u32,

    /// Timestamp of last sync.
    #[serde(default)]
    pub last_sync: Option<DateTime<Utc>>,

    /// Per-file lock entries.
    #[serde(default)]
    pub files: Vec<LockEntry>,
}

fn default_version() -> u32 {
    1
}

impl Default for LockFile {
    fn default() -> Self {
        Self {
            version: 1,
            last_sync: None,
            files: Vec::new(),
        }
    }
}

/// A single file entry in the lock file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LockEntry {
    /// Package name (e.g. "bash").
    pub package: String,

    /// Source file path relative to stow directory (e.g. "bash/.bashrc").
    pub source: String,

    /// SHA256 hash of the source file at last sync.
    pub source_hash: String,

    /// SHA256 hash of the resolved file at last sync.
    pub resolved_hash: String,

    /// Target path relative to target directory (e.g. ".bashrc").
    pub target: String,

    /// Tag expression strings that matched when this file was included.
    #[serde(default)]
    pub tags_matched: Vec<String>,
}

/// Drift state for a deployed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// Source and resolved hashes match lock — nothing to do.
    UpToDate,
    /// Source changed but resolved still matches lock — safe to re-resolve.
    SourceChanged,
    /// Resolved file was edited (drift) but source hasn't changed.
    Drifted,
    /// Both source and resolved changed — conflict.
    Conflict,
    /// Symlink is broken or target doesn't exist.
    Broken,
    /// File is in lock but package was removed from manifest.
    Stale,
    /// File is new (not in lock).
    New,
}

impl LockFile {
    /// Load a lock file from disk. Returns an empty lock if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| TowboatError::LockCorrupt(e.to_string()))
    }

    /// Save the lock file to disk.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            toml::to_string_pretty(self).map_err(|e| TowboatError::LockCorrupt(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Find a lock entry by package and source path.
    pub fn find(&self, package: &str, source: &str) -> Option<&LockEntry> {
        self.files
            .iter()
            .find(|e| e.package == package && e.source == source)
    }

    /// Find a lock entry by target path.
    pub fn find_by_target(&self, target: &str) -> Option<&LockEntry> {
        self.files.iter().find(|e| e.target == target)
    }

    /// Get all entries for a specific package.
    pub fn entries_for_package(&self, package: &str) -> Vec<&LockEntry> {
        self.files.iter().filter(|e| e.package == package).collect()
    }

    /// Remove all entries for a package.
    pub fn remove_package(&mut self, package: &str) {
        self.files.retain(|e| e.package != package);
    }

    /// Upsert a lock entry (update if exists, insert if new).
    pub fn upsert(&mut self, entry: LockEntry) {
        if let Some(existing) = self
            .files
            .iter_mut()
            .find(|e| e.package == entry.package && e.source == entry.source)
        {
            *existing = entry;
        } else {
            self.files.push(entry);
        }
    }
}

impl LockEntry {
    /// Determine the drift state of this entry given current hashes.
    pub fn state(&self, current_source_hash: &str, current_resolved_hash: &str) -> FileState {
        let source_changed = current_source_hash != self.source_hash;
        let resolved_changed = current_resolved_hash != self.resolved_hash;

        match (source_changed, resolved_changed) {
            (false, false) => FileState::UpToDate,
            (true, false) => FileState::SourceChanged,
            (false, true) => FileState::Drifted,
            (true, true) => FileState::Conflict,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_entry_state_up_to_date() {
        let entry = LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "abc".into(),
            resolved_hash: "def".into(),
            target: ".bashrc".into(),
            tags_matched: vec!["linux".into()],
        };
        assert_eq!(entry.state("abc", "def"), FileState::UpToDate);
    }

    #[test]
    fn lock_entry_state_source_changed() {
        let entry = LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "abc".into(),
            resolved_hash: "def".into(),
            target: ".bashrc".into(),
            tags_matched: vec![],
        };
        assert_eq!(entry.state("xyz", "def"), FileState::SourceChanged);
    }

    #[test]
    fn lock_entry_state_drifted() {
        let entry = LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "abc".into(),
            resolved_hash: "def".into(),
            target: ".bashrc".into(),
            tags_matched: vec![],
        };
        assert_eq!(entry.state("abc", "xyz"), FileState::Drifted);
    }

    #[test]
    fn lock_entry_state_conflict() {
        let entry = LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "abc".into(),
            resolved_hash: "def".into(),
            target: ".bashrc".into(),
            tags_matched: vec![],
        };
        assert_eq!(entry.state("xyz", "uvw"), FileState::Conflict);
    }

    #[test]
    fn lock_file_upsert_insert() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "abc".into(),
            resolved_hash: "def".into(),
            target: ".bashrc".into(),
            tags_matched: vec![],
        });
        assert_eq!(lock.files.len(), 1);
    }

    #[test]
    fn lock_file_upsert_update() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "abc".into(),
            resolved_hash: "def".into(),
            target: ".bashrc".into(),
            tags_matched: vec![],
        });
        lock.upsert(LockEntry {
            package: "bash".into(),
            source: "bash/.bashrc".into(),
            source_hash: "xyz".into(),
            resolved_hash: "uvw".into(),
            target: ".bashrc".into(),
            tags_matched: vec![],
        });
        assert_eq!(lock.files.len(), 1);
        assert_eq!(lock.files[0].source_hash, "xyz");
    }

    #[test]
    fn lock_file_find() {
        let lock = LockFile {
            files: vec![
                LockEntry {
                    package: "bash".into(),
                    source: "bash/.bashrc".into(),
                    source_hash: "abc".into(),
                    resolved_hash: "def".into(),
                    target: ".bashrc".into(),
                    tags_matched: vec![],
                },
                LockEntry {
                    package: "vim".into(),
                    source: "vim/.vimrc".into(),
                    source_hash: "ghi".into(),
                    resolved_hash: "jkl".into(),
                    target: ".vimrc".into(),
                    tags_matched: vec![],
                },
            ],
            ..Default::default()
        };
        assert!(lock.find("bash", "bash/.bashrc").is_some());
        assert!(lock.find("vim", "vim/.vimrc").is_some());
        assert!(lock.find("bash", "nonexistent").is_none());
    }

    #[test]
    fn lock_file_remove_package() {
        let mut lock = LockFile {
            files: vec![
                LockEntry {
                    package: "bash".into(),
                    source: "bash/.bashrc".into(),
                    source_hash: "abc".into(),
                    resolved_hash: "def".into(),
                    target: ".bashrc".into(),
                    tags_matched: vec![],
                },
                LockEntry {
                    package: "vim".into(),
                    source: "vim/.vimrc".into(),
                    source_hash: "ghi".into(),
                    resolved_hash: "jkl".into(),
                    target: ".vimrc".into(),
                    tags_matched: vec![],
                },
            ],
            ..Default::default()
        };
        lock.remove_package("bash");
        assert_eq!(lock.files.len(), 1);
        assert_eq!(lock.files[0].package, "vim");
    }

    #[test]
    fn lock_file_roundtrip() {
        let lock = LockFile {
            version: 1,
            last_sync: Some(Utc::now()),
            files: vec![LockEntry {
                package: "bash".into(),
                source: "bash/.bashrc".into(),
                source_hash: "abc123".into(),
                resolved_hash: "def456".into(),
                target: ".bashrc".into(),
                tags_matched: vec!["macos".into(), "laptop".into()],
            }],
        };
        let serialized = toml::to_string_pretty(&lock).unwrap();
        let deserialized: LockFile = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.files.len(), 1);
        assert_eq!(deserialized.files[0].source_hash, "abc123");
    }
}
