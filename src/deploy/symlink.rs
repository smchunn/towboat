//! Symlink CRUD: create, check, and remove symlinks from target to resolved files.

use std::fs;
use std::path::Path;

use crate::error::{Result, TowboatError};

/// Create a symlink at `link_path` pointing to `resolved_path`.
///
/// Creates parent directories as needed. Errors if the target already exists
/// unless `force` is true.
pub fn create_symlink(resolved_path: &Path, link_path: &Path, force: bool) -> Result<()> {
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if link_path.exists() || link_path.is_symlink() {
        if force {
            remove_symlink(link_path)?;
        } else {
            return Err(TowboatError::TargetExists(link_path.to_path_buf()));
        }
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(resolved_path, link_path).map_err(|e| {
        TowboatError::SymlinkFailed {
            link_source: resolved_path.to_path_buf(),
            link_target: link_path.to_path_buf(),
            reason: e.to_string(),
        }
    })?;

    #[cfg(windows)]
    std::os::windows::fs::symlink_file(resolved_path, link_path).map_err(|e| {
        TowboatError::SymlinkFailed {
            link_source: resolved_path.to_path_buf(),
            link_target: link_path.to_path_buf(),
            reason: e.to_string(),
        }
    })?;

    Ok(())
}

/// Remove a symlink (or file) at the given path.
/// Also cleans up empty parent directories.
pub fn remove_symlink(path: &Path) -> Result<()> {
    if path.is_symlink() || path.exists() {
        fs::remove_file(path)?;
    }

    // Clean up empty parent directories
    let mut dir = path.parent();
    while let Some(parent) = dir {
        if parent.read_dir()?.next().is_none() {
            fs::remove_dir(parent)?;
            dir = parent.parent();
        } else {
            break;
        }
    }

    Ok(())
}

/// Check if a symlink exists and points to the expected target.
pub fn symlink_matches(link_path: &Path, expected_target: &Path) -> bool {
    if !link_path.is_symlink() {
        return false;
    }
    match fs::read_link(link_path) {
        Ok(target) => target == expected_target,
        Err(_) => false,
    }
}

/// Check if a symlink is broken (exists as symlink but target doesn't exist).
pub fn is_broken_symlink(path: &Path) -> bool {
    path.is_symlink() && !path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_and_check_symlink() {
        let dir = TempDir::new().unwrap();
        let resolved = dir.path().join("resolved/test.txt");
        let link = dir.path().join("target/test.txt");

        fs::create_dir_all(resolved.parent().unwrap()).unwrap();
        fs::write(&resolved, "content").unwrap();

        create_symlink(&resolved, &link, false).unwrap();

        assert!(link.is_symlink());
        assert!(symlink_matches(&link, &resolved));
        assert_eq!(fs::read_to_string(&link).unwrap(), "content");
    }

    #[test]
    fn create_symlink_target_exists_error() {
        let dir = TempDir::new().unwrap();
        let resolved = dir.path().join("resolved.txt");
        let link = dir.path().join("link.txt");

        fs::write(&resolved, "resolved").unwrap();
        fs::write(&link, "existing").unwrap();

        let result = create_symlink(&resolved, &link, false);
        assert!(result.is_err());
    }

    #[test]
    fn create_symlink_force_overwrites() {
        let dir = TempDir::new().unwrap();
        let resolved = dir.path().join("resolved.txt");
        let link = dir.path().join("link.txt");

        fs::write(&resolved, "resolved").unwrap();
        fs::write(&link, "existing").unwrap();

        create_symlink(&resolved, &link, true).unwrap();
        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(&link).unwrap(), "resolved");
    }

    #[test]
    fn remove_symlink_and_cleanup() {
        let dir = TempDir::new().unwrap();
        let resolved = dir.path().join("resolved.txt");
        let nested_dir = dir.path().join("a/b/c");
        let link = nested_dir.join("link.txt");

        fs::write(&resolved, "content").unwrap();
        fs::create_dir_all(&nested_dir).unwrap();
        create_symlink(&resolved, &link, false).unwrap();
        assert!(link.exists());

        remove_symlink(&link).unwrap();
        assert!(!link.exists());
        // Parent directories should be cleaned up
        assert!(!nested_dir.exists());
    }

    #[test]
    fn symlink_matches_wrong_target() {
        let dir = TempDir::new().unwrap();
        let resolved_a = dir.path().join("a.txt");
        let resolved_b = dir.path().join("b.txt");
        let link = dir.path().join("link.txt");

        fs::write(&resolved_a, "a").unwrap();
        fs::write(&resolved_b, "b").unwrap();

        create_symlink(&resolved_a, &link, false).unwrap();
        assert!(symlink_matches(&link, &resolved_a));
        assert!(!symlink_matches(&link, &resolved_b));
    }

    #[test]
    fn broken_symlink_detection() {
        let dir = TempDir::new().unwrap();
        let resolved = dir.path().join("resolved.txt");
        let link = dir.path().join("link.txt");

        fs::write(&resolved, "content").unwrap();
        create_symlink(&resolved, &link, false).unwrap();

        // Remove the target to make the symlink broken
        fs::remove_file(&resolved).unwrap();
        assert!(is_broken_symlink(&link));

        // Regular file is not a broken symlink
        let regular = dir.path().join("regular.txt");
        fs::write(&regular, "content").unwrap();
        assert!(!is_broken_symlink(&regular));
    }

    #[test]
    fn symlink_matches_non_symlink() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();
        assert!(!symlink_matches(&file, &file));
    }

    #[test]
    fn create_symlink_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let resolved = dir.path().join("resolved.txt");
        let link = dir.path().join("deeply/nested/dir/link.txt");

        fs::write(&resolved, "content").unwrap();
        create_symlink(&resolved, &link, false).unwrap();

        assert!(link.is_symlink());
    }
}
