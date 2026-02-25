//! `towboat init` — scaffold a new `towboat.toml` manifest.

use std::path::Path;

use anyhow::Result;

pub fn run(stow_dir: &Path) -> Result<()> {
    let manifest_path = stow_dir.join("towboat.toml");

    if manifest_path.exists() {
        anyhow::bail!("towboat.toml already exists at {}", manifest_path.display());
    }

    let content = r#"[system]
tags = ["default"]

[variables]
# hostname = "my-machine"
# email = "user@example.com"

[packages]
# bash = {}
# vim = { tags = ["development"] }
"#;

    std::fs::write(&manifest_path, content)?;
    println!("Created {}", manifest_path.display());
    println!("\nNext steps:");
    println!("  1. Edit towboat.toml to set your system tags and packages");
    println!("  2. Create package directories with boat.toml files");
    println!("  3. Run `towboat sync` to deploy");

    Ok(())
}
