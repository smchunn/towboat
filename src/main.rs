use anyhow::Result;
use clap::{Arg, Command};
use std::path::PathBuf;
use towboat::{Config, run_towboat};

fn main() -> Result<()> {
    let matches = Command::new("towboat")
        .about("A stow-like tool for cross-platform dotfiles with build tags")
        .version("0.1.0")
        .arg(
            Arg::new("package")
                .help("Package directory to symlink (e.g., 'bash', 'vim', 'git')")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("dir")
                .short('d')
                .long("dir")
                .value_name("DIR")
                .help("Directory containing packages")
                .default_value("."),
        )
        .arg(
            Arg::new("target")
                .short('t')
                .long("target")
                .value_name("DIR")
                .help("Target directory to create symlinks in")
                .default_value("~"),
        )
        .arg(
            Arg::new("build")
                .short('b')
                .long("build")
                .value_name("TAG")
                .help("Build tag to match (defaults to 'default' if not specified)")
                .required(false),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Show what would be done without making changes")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .short('f')
                .long("force")
                .help("Overwrite existing files in target directory")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("adopt")
                .long("adopt")
                .help("Adopt existing files from target directory back to package")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("remove")
                .short('r')
                .long("remove")
                .help("Remove symlinks/files for this package from target directory")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let package_name = matches.get_one::<String>("package").unwrap();
    let packages_dir = PathBuf::from(matches.get_one::<String>("dir").unwrap());
    let package = packages_dir.join(package_name);

    // Expand ~ in target path
    let target_str = matches.get_one::<String>("target").unwrap();
    let target_dir = if target_str == "~" {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home),
            Err(_) => PathBuf::from("."),
        }
    } else {
        PathBuf::from(target_str)
    };

    let build_tag = matches
        .get_one::<String>("build")
        .map(ToString::to_string)
        .unwrap_or_else(|| "default".to_string());

    let config = Config {
        package,
        target_dir,
        build_tag,
        dry_run: matches.get_flag("dry-run"),
        force: matches.get_flag("force"),
        adopt: matches.get_flag("adopt"),
        remove: matches.get_flag("remove"),
    };

    run_towboat(config)
}
