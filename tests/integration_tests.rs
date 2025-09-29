use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("A stow-like tool for cross-platform dotfiles"));
}

#[test]
fn test_cli_missing_required_args() {
    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_cli_nonexistent_source_directory() {
    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args(["-d", "/nonexistent", "package"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Source directory does not exist"));
}

#[test]
fn test_cli_dry_run_mode() {
    let temp_dir = TempDir::new().unwrap();
    let stow_dir = temp_dir.path();
    let package_dir = stow_dir.join("testpackage");
    let target_dir = temp_dir.path().join("target");

    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Create a test file with build tags
    fs::write(
        package_dir.join(".bashrc"),
        r#"# Common content
export PATH=$PATH:/usr/local/bin

# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args([
        "-d",
        stow_dir.to_str().unwrap(),
        "-t",
        target_dir.to_str().unwrap(),
        "-b",
        "linux",
        "--dry-run",
        "testpackage",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"))
        .stdout(predicate::str::contains("Would create processed file"))
        .stdout(predicate::str::contains("Dry run completed"));

    // Verify no files were actually created
    assert!(!target_dir.join(".bashrc").exists());
}

#[test]
fn test_cli_successful_run_with_build_tags() {
    let temp_dir = TempDir::new().unwrap();
    let stow_dir = temp_dir.path();
    let package_dir = stow_dir.join("testpackage");
    let target_dir = temp_dir.path().join("target");

    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Create a test file with build tags
    fs::write(
        package_dir.join(".bashrc"),
        r#"# Common content
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
echo "Hello from shell""#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args([
        "-d",
        stow_dir.to_str().unwrap(),
        "-t",
        target_dir.to_str().unwrap(),
        "-b",
        "linux",
        "testpackage",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found 1 matching files"))
        .stdout(predicate::str::contains("Created processed file"))
        .stdout(predicate::str::contains("Completed successfully"));

    // Verify the processed file was created correctly
    let processed_content = fs::read_to_string(target_dir.join(".bashrc")).unwrap();
    assert!(processed_content.contains("alias ls='ls --color=auto'"));
    assert!(processed_content.contains("export EDITOR=vim"));
    assert!(!processed_content.contains("alias ls='ls -G'"));
    assert!(!processed_content.contains("export EDITOR=nano"));
    assert!(processed_content.contains("# Common content"));
    assert!(processed_content.contains("echo \"Hello from shell\""));
}

#[test]
fn test_cli_filename_based_matching() {
    let temp_dir = TempDir::new().unwrap();
    let stow_dir = temp_dir.path();
    let package_dir = stow_dir.join("testpackage");
    let target_dir = temp_dir.path().join("target");

    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Create files with build tags in filenames
    fs::write(package_dir.join(".vimrc.linux"), "set number\nset autoindent").unwrap();
    fs::write(package_dir.join(".vimrc.macos"), "set bg=dark\nset mouse=a").unwrap();
    fs::write(package_dir.join(".gitconfig"), "common git config").unwrap();

    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args([
        "-d",
        stow_dir.to_str().unwrap(),
        "-t",
        target_dir.to_str().unwrap(),
        "-b",
        "linux",
        "testpackage",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found 1 matching files"));

    // Verify only the linux file was processed and renamed correctly
    assert!(target_dir.join(".vimrc").exists());
    assert!(!target_dir.join(".vimrc.linux").exists());
    assert!(!target_dir.join(".vimrc.macos").exists());
    assert!(!target_dir.join(".gitconfig").exists());

    let content = fs::read_to_string(target_dir.join(".vimrc")).unwrap();
    assert!(content.contains("set number"));
    assert!(content.contains("set autoindent"));
}

#[test]
fn test_cli_nested_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let stow_dir = temp_dir.path();
    let package_dir = stow_dir.join("testpackage");
    let target_dir = temp_dir.path().join("target");

    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Create nested directory structure
    let config_dir = package_dir.join(".config").join("app");
    fs::create_dir_all(&config_dir).unwrap();

    fs::write(
        config_dir.join("config.toml.linux"),
        "[app]\ntheme = \"dark\"\nshell = \"/bin/bash\"",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args([
        "-d",
        stow_dir.to_str().unwrap(),
        "-t",
        target_dir.to_str().unwrap(),
        "-b",
        "linux",
        "testpackage",
    ]);

    cmd.assert().success();

    // Verify nested directory structure was created
    let target_config_path = target_dir.join(".config").join("app").join("config.toml");
    assert!(target_config_path.exists());

    let content = fs::read_to_string(&target_config_path).unwrap();
    assert!(content.contains("theme = \"dark\""));
    assert!(content.contains("shell = \"/bin/bash\""));
}

#[test]
fn test_cli_no_matching_files() {
    let temp_dir = TempDir::new().unwrap();
    let stow_dir = temp_dir.path();
    let package_dir = stow_dir.join("testpackage");
    let target_dir = temp_dir.path().join("target");

    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Create files that don't match the build tag
    fs::write(package_dir.join(".vimrc.macos"), "macos content").unwrap();
    fs::write(package_dir.join("README.md"), "documentation").unwrap();

    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args([
        "-d",
        stow_dir.to_str().unwrap(),
        "-t",
        target_dir.to_str().unwrap(),
        "-b",
        "linux",
        "testpackage",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No files found matching build tag 'linux'"));
}

#[test]
fn test_cli_mixed_file_types() {
    let temp_dir = TempDir::new().unwrap();
    let stow_dir = temp_dir.path();
    let package_dir = stow_dir.join("testpackage");
    let target_dir = temp_dir.path().join("target");

    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Mix of filename-based and content-based matching
    fs::write(package_dir.join(".bashrc.linux"), "bash linux config").unwrap();

    fs::write(
        package_dir.join(".profile"),
        r#"# Common profile
export USER_HOME=$HOME

# {linux-
export DISPLAY=:0
# -linux}

# {macos-
export HOMEBREW_PREFIX=/usr/local
# -macos}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("towboat").unwrap();
    cmd.args([
        "-d",
        stow_dir.to_str().unwrap(),
        "-t",
        target_dir.to_str().unwrap(),
        "-b",
        "linux",
        "testpackage",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found 2 matching files"));

    // Verify both files were processed
    assert!(target_dir.join(".bashrc").exists());
    assert!(target_dir.join(".profile").exists());

    // Verify content processing
    let profile_content = fs::read_to_string(target_dir.join(".profile")).unwrap();
    assert!(profile_content.contains("export DISPLAY=:0"));
    assert!(!profile_content.contains("export HOMEBREW_PREFIX"));
}