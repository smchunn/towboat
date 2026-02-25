//! End-to-end integration tests for towboat v2.

use std::collections::{HashMap, HashSet};
use std::fs;
use tempfile::TempDir;

/// Create a complete stow directory with manifest, packages, and configs.
fn setup_stow_dir() -> TempDir {
    let dir = TempDir::new().unwrap();

    // Create towboat.toml manifest
    fs::write(
        dir.path().join("towboat.toml"),
        r#"
[system]
tags = ["linux", "laptop", "work"]

[variables]
hostname = "workbox"
email = "user@work.com"

[packages]
bash = {}
git = {}
vim = { tags = ["laptop"] }
"#,
    )
    .unwrap();

    // Create bash package
    let bash_dir = dir.path().join("bash");
    fs::create_dir_all(&bash_dir).unwrap();
    fs::write(
        bash_dir.join("boat.toml"),
        r#"
[targets]
".bashrc" = { tags = "linux" }
".profile" = { tags = "linux | macos" }
"#,
    )
    .unwrap();
    fs::write(
        bash_dir.join(".bashrc"),
        r#"#!/bin/bash
# Common
export PATH=$PATH:/usr/local/bin

# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}
"#,
    )
    .unwrap();
    fs::write(bash_dir.join(".profile"), "source ~/.bashrc\n").unwrap();

    // Create git package
    let git_dir = dir.path().join("git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("boat.toml"),
        r#"
[targets]
".gitconfig" = { tags = ["linux", "macos"] }
"#,
    )
    .unwrap();
    fs::write(
        git_dir.join(".gitconfig"),
        "[user]\n    name = {{ hostname }}\n    email = {{ email }}\n",
    )
    .unwrap();

    // Create vim package
    let vim_dir = dir.path().join("vim");
    fs::create_dir_all(&vim_dir).unwrap();
    fs::write(
        vim_dir.join("boat.toml"),
        r#"
[targets]
".vimrc" = { tags = ["linux", "macos"] }
"#,
    )
    .unwrap();
    fs::write(vim_dir.join(".vimrc"), "set number\nset ruler\n").unwrap();

    dir
}

#[test]
fn full_sync_workflow() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(
        stow.path(),
        target.path(),
        None, // all packages
        false,
        false,
    )
    .unwrap();

    // Verify .bashrc was resolved with linux tags
    let bashrc = target.path().join(".bashrc");
    assert!(bashrc.is_symlink(), ".bashrc should be a symlink");
    let content = fs::read_to_string(&bashrc).unwrap();
    assert!(content.contains("--color=auto"), "Should have linux alias");
    assert!(!content.contains("-G"), "Should not have macos alias");
    assert!(
        content.contains("export PATH"),
        "Should have common content"
    );

    // Verify .profile was symlinked
    let profile = target.path().join(".profile");
    assert!(profile.is_symlink());

    // Verify .gitconfig has templates resolved
    let gitconfig = target.path().join(".gitconfig");
    assert!(gitconfig.is_symlink());
    let content = fs::read_to_string(&gitconfig).unwrap();
    assert!(content.contains("name = workbox"));
    assert!(content.contains("email = user@work.com"));

    // Verify .vimrc (vim package has tags = ["laptop"] which is in active tags)
    let vimrc = target.path().join(".vimrc");
    assert!(vimrc.is_symlink());

    // Verify lock file was created
    let lock_path = stow.path().join(".towboat/towboat.lock");
    assert!(lock_path.exists());

    // Verify resolved directory structure
    let resolved_dir = stow.path().join(".towboat/resolved");
    assert!(resolved_dir.join("bash/.bashrc").exists());
    assert!(resolved_dir.join("bash/.profile").exists());
    assert!(resolved_dir.join("git/.gitconfig").exists());
    assert!(resolved_dir.join("vim/.vimrc").exists());
}

#[test]
fn sync_single_package() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    towboat::commands::sync::run(stow.path(), target.path(), Some("bash"), false, false).unwrap();

    assert!(target.path().join(".bashrc").is_symlink());
    assert!(target.path().join(".profile").is_symlink());
    assert!(!target.path().join(".gitconfig").exists());
    assert!(!target.path().join(".vimrc").exists());
}

#[test]
fn sync_nonexistent_package_errors() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    let result = towboat::commands::sync::run(
        stow.path(),
        target.path(),
        Some("nonexistent"),
        false,
        false,
    );
    assert!(result.is_err());
}

#[test]
fn sync_dry_run_no_changes() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    towboat::commands::sync::run(stow.path(), target.path(), None, true, false).unwrap();

    // Nothing should be created
    assert!(!target.path().join(".bashrc").exists());
    assert!(!target.path().join(".gitconfig").exists());
}

#[test]
fn sync_idempotent() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Second sync should succeed without force
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Files should still be correct
    let content = fs::read_to_string(target.path().join(".bashrc")).unwrap();
    assert!(content.contains("--color=auto"));
}

#[test]
fn sync_force_overwrites() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // Create a conflicting file
    fs::write(target.path().join(".bashrc"), "existing content").unwrap();

    // Sync with force should succeed
    towboat::commands::sync::run(stow.path(), target.path(), None, false, true).unwrap();

    let content = fs::read_to_string(target.path().join(".bashrc")).unwrap();
    assert!(content.contains("--color=auto"));
}

#[test]
fn sync_detects_source_change() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Modify source
    fs::write(
        stow.path().join("bash/.bashrc"),
        r#"#!/bin/bash
export PATH=$PATH:/usr/local/bin
# {linux-
alias ls='ls --color=auto'
alias ll='ls -la'
# -linux}
"#,
    )
    .unwrap();

    // Re-sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Should include the new alias
    let content = fs::read_to_string(target.path().join(".bashrc")).unwrap();
    assert!(content.contains("alias ll='ls -la'"));
}

#[test]
fn init_creates_manifest() {
    let dir = TempDir::new().unwrap();

    towboat::commands::init::run(dir.path()).unwrap();

    let manifest_path = dir.path().join("towboat.toml");
    assert!(manifest_path.exists());

    let content = fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("[system]"));
    assert!(content.contains("[packages]"));
}

#[test]
fn init_fails_if_exists() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("towboat.toml"), "existing").unwrap();

    let result = towboat::commands::init::run(dir.path());
    assert!(result.is_err());
}

#[test]
fn resolve_file_api() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.conf");
    fs::write(
        &file,
        "host = {{ hostname }}\n# {linux-\npath = /usr\n# -linux}\n",
    )
    .unwrap();

    let tags: HashSet<String> = ["linux"].iter().map(|s| s.to_string()).collect();
    let vars: HashMap<String, String> = [("hostname", "myhost")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let (content, had_tags) = towboat::resolve_file(&file, &tags, &vars).unwrap();

    assert!(had_tags);
    assert!(content.contains("host = myhost"));
    assert!(content.contains("path = /usr"));
}

#[test]
fn status_with_no_lock_file() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // Status before any sync should work gracefully
    towboat::commands::status::run(stow.path(), target.path(), None).unwrap();
}

#[test]
fn full_lifecycle_sync_status_diff() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // Sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Status should work
    towboat::commands::status::run(stow.path(), target.path(), None).unwrap();

    // Diff should work (nothing changed)
    towboat::commands::diff::run(stow.path(), target.path(), None).unwrap();

    // Modify source and check diff
    fs::write(
        stow.path().join("bash/.bashrc"),
        "#!/bin/bash\n# {linux-\nnew content\n# -linux}\n",
    )
    .unwrap();

    towboat::commands::diff::run(stow.path(), target.path(), None).unwrap();
}

#[test]
fn cli_help_works() {
    use assert_cmd::Command;

    Command::cargo_bin("towboat")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("dotfile manager"));
}

#[test]
fn cli_init_subcommand() {
    use assert_cmd::Command;

    let dir = TempDir::new().unwrap();

    Command::cargo_bin("towboat")
        .unwrap()
        .args(["--dir", dir.path().to_str().unwrap(), "init"])
        .assert()
        .success();

    assert!(dir.path().join("towboat.toml").exists());
}

#[test]
fn cli_sync_dry_run() {
    use assert_cmd::Command;

    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    Command::cargo_bin("towboat")
        .unwrap()
        .args([
            "--dir",
            stow.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
            "sync",
            "--dry-run",
        ])
        .assert()
        .success();

    // Nothing should be created during dry run
    assert!(!target.path().join(".bashrc").exists());
}

// --- Edge case tests ---

#[test]
fn sync_package_removed_from_manifest_cleans_up() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync — all packages deployed
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();
    assert!(target.path().join(".vimrc").is_symlink());

    // Remove vim from manifest
    fs::write(
        stow.path().join("towboat.toml"),
        r#"
[system]
tags = ["linux", "laptop", "work"]

[variables]
hostname = "workbox"
email = "user@work.com"

[packages]
bash = {}
git = {}
"#,
    )
    .unwrap();

    // Re-sync — vim should be cleaned up
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();
    assert!(
        !target.path().join(".vimrc").exists(),
        ".vimrc should be removed after vim is dropped from manifest"
    );
}

#[test]
fn sync_file_removed_from_boat_toml_cleans_up() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();
    assert!(target.path().join(".profile").is_symlink());

    // Remove .profile from boat.toml targets
    fs::write(
        stow.path().join("bash/boat.toml"),
        r#"
[targets]
".bashrc" = { tags = "linux" }
"#,
    )
    .unwrap();

    // Re-sync bash
    towboat::commands::sync::run(stow.path(), target.path(), Some("bash"), false, false).unwrap();
    assert!(
        !target.path().join(".profile").exists(),
        ".profile should be removed after dropping from boat.toml"
    );
}

#[test]
fn sync_package_tag_requirement_not_met_skips() {
    let dir = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    fs::write(
        dir.path().join("towboat.toml"),
        r#"
[system]
tags = ["linux"]

[packages]
restricted = { tags = ["server"] }
"#,
    )
    .unwrap();

    let pkg = dir.path().join("restricted");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("boat.toml"),
        r#"
[targets]
".config" = { tags = ["linux"] }
"#,
    )
    .unwrap();
    fs::write(pkg.join(".config"), "server only\n").unwrap();

    towboat::commands::sync::run(dir.path(), target.path(), None, false, false).unwrap();

    assert!(
        !target.path().join(".config").exists(),
        "Package with unmet tag requirement should not deploy"
    );
}

#[test]
fn sync_target_remap() {
    let dir = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    fs::write(
        dir.path().join("towboat.toml"),
        r#"
[system]
tags = ["linux"]

[packages]
myapp = {}
"#,
    )
    .unwrap();

    let pkg = dir.path().join("myapp");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("boat.toml"),
        r#"
[targets]
"dev-config.sh" = { target = ".config.sh", tags = "linux" }
"#,
    )
    .unwrap();
    fs::write(pkg.join("dev-config.sh"), "export DEV=1\n").unwrap();

    towboat::commands::sync::run(dir.path(), target.path(), None, false, false).unwrap();

    assert!(
        target.path().join(".config.sh").is_symlink(),
        "Remapped target should be used"
    );
    assert!(
        !target.path().join("dev-config.sh").exists(),
        "Original name should not appear in target"
    );
    let content = fs::read_to_string(target.path().join(".config.sh")).unwrap();
    assert!(content.contains("export DEV=1"));
}

#[test]
fn sync_default_include_all() {
    let dir = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    fs::write(
        dir.path().join("towboat.toml"),
        r#"
[system]
tags = ["default"]

[packages]
misc = {}
"#,
    )
    .unwrap();

    let pkg = dir.path().join("misc");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("boat.toml"),
        r#"
[targets]

[default]
include_all = true
default_tag = "default"
"#,
    )
    .unwrap();
    fs::write(pkg.join(".aliasrc"), "alias hi='echo hi'\n").unwrap();
    fs::write(pkg.join(".envrc"), "export FOO=1\n").unwrap();

    towboat::commands::sync::run(dir.path(), target.path(), None, false, false).unwrap();

    assert!(
        target.path().join(".aliasrc").is_symlink(),
        "Unconfigured files should be included with include_all"
    );
    assert!(
        target.path().join(".envrc").is_symlink(),
        "Unconfigured files should be included with include_all"
    );
}

#[test]
fn sync_with_nested_directory() {
    let dir = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    fs::write(
        dir.path().join("towboat.toml"),
        r#"
[system]
tags = ["linux"]

[packages]
config = {}
"#,
    )
    .unwrap();

    let pkg = dir.path().join("config");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("boat.toml"),
        r#"
[targets]
".config/myapp" = { tags = ["linux"] }
"#,
    )
    .unwrap();
    let nested = pkg.join(".config/myapp");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("settings.toml"), "theme = \"dark\"\n").unwrap();
    fs::write(nested.join("keybinds.toml"), "save = \"ctrl+s\"\n").unwrap();

    towboat::commands::sync::run(dir.path(), target.path(), None, false, false).unwrap();

    assert!(
        target
            .path()
            .join(".config/myapp/settings.toml")
            .is_symlink()
    );
    assert!(
        target
            .path()
            .join(".config/myapp/keybinds.toml")
            .is_symlink()
    );

    let content = fs::read_to_string(target.path().join(".config/myapp/settings.toml")).unwrap();
    assert!(content.contains("dark"));
}

#[test]
fn resolve_with_both_tags_and_templates() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.conf");
    fs::write(
        &file,
        "host = {{ hostname }}\n# {linux-\npath = /usr\nemail = {{ email }}\n# -linux}\n# {macos-\npath = /opt\n# -macos}\n",
    )
    .unwrap();

    let tags: HashSet<String> = ["linux"].iter().map(|s| s.to_string()).collect();
    let vars: HashMap<String, String> = [("hostname", "box"), ("email", "a@b.com")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let (content, had_tags) = towboat::resolve_file(&file, &tags, &vars).unwrap();

    assert!(had_tags);
    assert!(content.contains("host = box"));
    assert!(content.contains("path = /usr"));
    assert!(content.contains("email = a@b.com"));
    assert!(!content.contains("/opt"));
}

#[test]
fn resolve_undefined_variable_errors() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.conf");
    fs::write(&file, "value = {{ undefined_var }}\n").unwrap();

    let tags: HashSet<String> = HashSet::new();
    let vars: HashMap<String, String> = HashMap::new();

    let result = towboat::resolve_file(&file, &tags, &vars);
    assert!(result.is_err());
}

#[test]
fn sync_conflict_detection_without_force() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Modify BOTH source AND resolved file to create a conflict
    fs::write(
        stow.path().join("bash/.bashrc"),
        "#!/bin/bash\nnew source content\n",
    )
    .unwrap();

    let resolved_bashrc = stow.path().join(".towboat/resolved/bash/.bashrc");
    fs::write(&resolved_bashrc, "manually edited resolved file\n").unwrap();

    // Re-sync without force should report conflict
    let result = towboat::commands::sync::run(stow.path(), target.path(), None, false, false);
    assert!(result.is_err(), "Should error on conflict without --force");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("conflict"),
        "Error should mention conflict: {err}"
    );
}

#[test]
fn sync_conflict_resolved_with_force() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Create a conflict (both source and resolved changed)
    fs::write(
        stow.path().join("bash/.bashrc"),
        "#!/bin/bash\nnew source\n",
    )
    .unwrap();
    let resolved_bashrc = stow.path().join(".towboat/resolved/bash/.bashrc");
    fs::write(&resolved_bashrc, "edited resolved\n").unwrap();

    // Force should overwrite
    towboat::commands::sync::run(stow.path(), target.path(), None, false, true).unwrap();

    let content = fs::read_to_string(target.path().join(".bashrc")).unwrap();
    assert!(
        content.contains("new source"),
        "Force should re-resolve from source"
    );
}

#[test]
fn sync_preserves_drifted_resolved_file() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    // First sync
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    // Edit only the resolved file (user edits via symlink)
    let resolved_profile = stow.path().join(".towboat/resolved/bash/.profile");
    fs::write(&resolved_profile, "user edited this\n").unwrap();

    // Re-sync without force — should preserve the drift
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    let content = fs::read_to_string(target.path().join(".profile")).unwrap();
    assert_eq!(
        content, "user edited this\n",
        "Drifted file should be preserved when source hasn't changed"
    );
}

#[test]
fn lock_file_persists_across_syncs() {
    let stow = setup_stow_dir();
    let target = TempDir::new().unwrap();

    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    let lock_content = fs::read_to_string(stow.path().join(".towboat/towboat.lock")).unwrap();
    assert!(lock_content.contains("version = 1"));
    assert!(lock_content.contains("last_sync"));
    assert!(lock_content.contains("bash/.bashrc"));

    // Second sync should update lock
    towboat::commands::sync::run(stow.path(), target.path(), None, false, false).unwrap();

    let lock_content2 = fs::read_to_string(stow.path().join(".towboat/towboat.lock")).unwrap();
    assert!(lock_content2.contains("version = 1"));
}
