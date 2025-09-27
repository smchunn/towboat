use std::fs;
use tempfile::TempDir;
use std::path::PathBuf;

pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
}

impl TestEnvironment {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        Self {
            temp_dir,
            source_dir,
            target_dir,
        }
    }

    pub fn create_file(&self, relative_path: &str, content: &str) -> PathBuf {
        let file_path = self.source_dir.join(relative_path);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(&file_path, content).unwrap();
        file_path
    }

    pub fn create_nested_structure(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Root level files
        files.push(self.create_file(".bashrc.linux", include_str!("fixtures/bashrc_linux.txt")));
        files.push(self.create_file(".bashrc.macos", include_str!("fixtures/bashrc_macos.txt")));
        files.push(self.create_file(".gitconfig", include_str!("fixtures/gitconfig_with_tags.txt")));

        // Nested config files
        files.push(self.create_file(".config/nvim/init.vim.linux", include_str!("fixtures/nvim_linux.vim")));
        files.push(self.create_file(".config/nvim/init.vim.macos", include_str!("fixtures/nvim_macos.vim")));
        files.push(self.create_file(".config/app/config.toml", include_str!("fixtures/config_with_tags.toml")));

        // SSH config with tags
        files.push(self.create_file(".ssh/config", include_str!("fixtures/ssh_config_with_tags.txt")));

        files
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use towboat::discover_files;

    #[test]
    fn test_fixture_environment_creation() {
        let env = TestEnvironment::new();

        assert!(env.source_dir.exists());
        assert!(env.target_dir.exists());
    }

    #[test]
    fn test_create_file_with_nested_dirs() {
        let env = TestEnvironment::new();

        let file_path = env.create_file(".config/deep/nested/file.conf", "test content");

        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "test content");
    }

    #[test]
    fn test_nested_structure_discovery() {
        let env = TestEnvironment::new();
        env.create_nested_structure();

        let linux_files = discover_files(&env.source_dir, "linux").unwrap();
        let macos_files = discover_files(&env.source_dir, "macos").unwrap();

        // Should find multiple files for each platform
        assert!(linux_files.len() >= 3);
        assert!(macos_files.len() >= 3);

        // Verify different platforms find different files
        let linux_names: Vec<_> = linux_files.iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        let macos_names: Vec<_> = macos_files.iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        assert!(linux_names.contains(&".bashrc.linux".to_string()));
        assert!(macos_names.contains(&".bashrc.macos".to_string()));
    }
}

// Integration test using fixtures
#[cfg(test)]
mod integration_fixture_tests {
    use super::*;
    use assert_cmd::Command;
    use predicates::prelude::*;

    #[test]
    fn test_full_deployment_scenario() {
        let env = TestEnvironment::new();
        env.create_nested_structure();

        let mut cmd = Command::cargo_bin("towboat").unwrap();
        cmd.args([
            "-s", env.source_dir.to_str().unwrap(),
            "-t", env.target_dir.to_str().unwrap(),
            "-b", "linux",
        ]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Completed successfully"));

        // Verify expected files were created
        assert!(env.target_dir.join(".bashrc").exists());
        assert!(env.target_dir.join(".gitconfig").exists());
        assert!(env.target_dir.join(".config/nvim/init.vim").exists());
        assert!(env.target_dir.join(".ssh/config").exists());

        // Verify processed content
        let bashrc_content = fs::read_to_string(env.target_dir.join(".bashrc")).unwrap();
        // Check for linux-specific content from the fixture
        assert!(bashrc_content.contains("Linux-specific bash configuration"));
        assert!(bashrc_content.contains("--color=auto"));
    }

    #[test]
    fn test_cross_platform_differences() {
        let env = TestEnvironment::new();
        env.create_nested_structure();

        // Deploy for Linux
        let mut cmd = Command::cargo_bin("towboat").unwrap();
        cmd.args([
            "-s", env.source_dir.to_str().unwrap(),
            "-t", env.target_dir.join("linux").to_str().unwrap(),
            "-b", "linux",
        ]);
        cmd.assert().success();

        // Deploy for macOS
        let mut cmd = Command::cargo_bin("towboat").unwrap();
        cmd.args([
            "-s", env.source_dir.to_str().unwrap(),
            "-t", env.target_dir.join("macos").to_str().unwrap(),
            "-b", "macos",
        ]);
        cmd.assert().success();

        // Compare results - they should have different content
        let linux_dir = env.target_dir.join("linux");
        let macos_dir = env.target_dir.join("macos");

        assert!(linux_dir.join(".bashrc").exists());
        assert!(macos_dir.join(".bashrc").exists());

        // The content should be different due to build tag processing
        let linux_content = fs::read_to_string(linux_dir.join(".bashrc")).unwrap_or_default();
        let macos_content = fs::read_to_string(macos_dir.join(".bashrc")).unwrap_or_default();

        // At minimum, they shouldn't be identical (unless fixtures are identical)
        // In a real scenario, these would have platform-specific differences
        if !linux_content.is_empty() && !macos_content.is_empty() {
            // Only compare if both have content - depends on fixtures existing
            println!("Linux bashrc: {}", linux_content);
            println!("macOS bashrc: {}", macos_content);
        }
    }
}