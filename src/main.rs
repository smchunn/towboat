use anyhow::Result;
use clap::{Arg, Command};
use std::path::PathBuf;
use towboat::{Config, run_towboat};

fn main() -> Result<()> {
    let matches = Command::new("towboat")
        .about("A stow-like tool for cross-platform dotfiles with build tags")
        .version("0.1.0")
        .arg(
            Arg::new("source")
                .short('s')
                .long("source")
                .value_name("DIR")
                .help("Source directory containing dotfiles")
                .required(true),
        )
        .arg(
            Arg::new("target")
                .short('t')
                .long("target")
                .value_name("DIR")
                .help("Target directory to create symlinks in")
                .default_value("."),
        )
        .arg(
            Arg::new("build")
                .short('b')
                .long("build")
                .value_name("TAG")
                .help("Build tag to match (e.g., 'linux', 'macos', 'windows')")
                .required(true),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Show what would be done without making changes")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let config = Config {
        source_dir: PathBuf::from(matches.get_one::<String>("source").unwrap()),
        target_dir: PathBuf::from(matches.get_one::<String>("target").unwrap()),
        build_tag: matches.get_one::<String>("build").unwrap().clone(),
        dry_run: matches.get_flag("dry-run"),
    };

    run_towboat(config)
}
