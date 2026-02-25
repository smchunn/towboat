//! Towboat CLI — subcommand-based interface for dotfile management.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "towboat",
    version,
    about = "A cross-platform dotfile manager with build tags"
)]
struct Cli {
    /// Stow directory containing packages and towboat.toml
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,

    /// Target directory to create symlinks in
    #[arg(short, long, default_value = "~")]
    target: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Resolve packages and create/update symlinks
    Sync {
        /// Specific package to sync (syncs all if omitted)
        package: Option<String>,
        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
        /// Overwrite existing files and resolve conflicts
        #[arg(short, long)]
        force: bool,
    },
    /// Show per-file state (up-to-date, source-changed, drifted, conflict, broken)
    Status {
        /// Specific package to check (checks all if omitted)
        package: Option<String>,
    },
    /// Show what would change on next sync
    Diff {
        /// Specific package to diff (diffs all if omitted)
        package: Option<String>,
    },
    /// Scaffold a new towboat.toml manifest
    Init,
}

fn expand_tilde(path: &Path) -> PathBuf {
    if path.to_string_lossy() == "~" {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home),
            Err(_) => path.to_path_buf(),
        }
    } else if let Some(rest) = path.to_string_lossy().strip_prefix("~/") {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(rest),
            Err(_) => path.to_path_buf(),
        }
    } else {
        path.to_path_buf()
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let stow_dir = cli.dir.canonicalize().unwrap_or(cli.dir);
    let target_dir = expand_tilde(&cli.target);

    match cli.command {
        Commands::Sync {
            package,
            dry_run,
            force,
        } => {
            towboat::commands::sync::run(&stow_dir, &target_dir, package.as_deref(), dry_run, force)
        }
        Commands::Status { package } => {
            towboat::commands::status::run(&stow_dir, &target_dir, package.as_deref())
        }
        Commands::Diff { package } => {
            towboat::commands::diff::run(&stow_dir, &target_dir, package.as_deref())
        }
        Commands::Init => towboat::commands::init::run(&stow_dir),
    }
}
