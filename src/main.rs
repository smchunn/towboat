use anyhow::Result;
use clap::{Arg, Command};
use std::path::PathBuf;
use towboat::{Config, run_towboat, find_boat_config, parse_boat_config};

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
                .help("Stow directory containing packages")
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
                .help("Build tag to match (e.g., 'linux', 'macos', 'windows')")
                .required(false),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Show what would be done without making changes")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let package_name = matches.get_one::<String>("package").unwrap();
    let stow_dir = PathBuf::from(matches.get_one::<String>("dir").unwrap());
    let source_dir = stow_dir.join(package_name);

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

    // Read boat.toml from package directory if it exists
    let (final_target_dir, build_tag) = if let Some(config_path) = find_boat_config(&source_dir) {
        match parse_boat_config(&config_path) {
            Ok(boat_config) => {
                let config_target_dir = if let Some(config_target) = boat_config.target_dir {
                    // Expand ~ in config target path
                    if config_target == "~" {
                        match std::env::var("HOME") {
                            Ok(home) => PathBuf::from(home),
                            Err(_) => target_dir.clone(),
                        }
                    } else if config_target.starts_with("~/") {
                        match std::env::var("HOME") {
                            Ok(home) => PathBuf::from(home).join(&config_target[2..]),
                            Err(_) => PathBuf::from(&config_target[2..]),
                        }
                    } else {
                        PathBuf::from(config_target)
                    }
                } else {
                    target_dir.clone()
                };

                let config_build_tag = if let Some(cli_build_tag) = matches.get_one::<String>("build") {
                    cli_build_tag.clone()
                } else if let Some(default_tags) = boat_config.build_tags {
                    // Use first default build tag from config
                    default_tags.into_iter().next().unwrap_or_else(|| {
                        // Fallback to platform detection
                        if cfg!(target_os = "linux") {
                            "linux".to_string()
                        } else if cfg!(target_os = "macos") {
                            "macos".to_string()
                        } else if cfg!(target_os = "windows") {
                            "windows".to_string()
                        } else {
                            "default".to_string()
                        }
                    })
                } else {
                    // Fallback to platform detection
                    if cfg!(target_os = "linux") {
                        "linux".to_string()
                    } else if cfg!(target_os = "macos") {
                        "macos".to_string()
                    } else if cfg!(target_os = "windows") {
                        "windows".to_string()
                    } else {
                        "default".to_string()
                    }
                };

                (config_target_dir, config_build_tag)
            }
            Err(_) => {
                // If boat.toml exists but can't be parsed, use CLI defaults
                let build_tag = matches.get_one::<String>("build")
                    .map(|s| s.clone())
                    .unwrap_or_else(|| {
                        if cfg!(target_os = "linux") {
                            "linux".to_string()
                        } else if cfg!(target_os = "macos") {
                            "macos".to_string()
                        } else if cfg!(target_os = "windows") {
                            "windows".to_string()
                        } else {
                            "default".to_string()
                        }
                    });
                (target_dir, build_tag)
            }
        }
    } else {
        // No boat.toml found, use CLI arguments and platform defaults
        let build_tag = matches.get_one::<String>("build")
            .map(|s| s.clone())
            .unwrap_or_else(|| {
                if cfg!(target_os = "linux") {
                    "linux".to_string()
                } else if cfg!(target_os = "macos") {
                    "macos".to_string()
                } else if cfg!(target_os = "windows") {
                    "windows".to_string()
                } else {
                    "default".to_string()
                }
            });
        (target_dir, build_tag)
    };

    let config = Config {
        source_dir,
        target_dir: final_target_dir,
        build_tag,
        dry_run: matches.get_flag("dry-run"),
    };

    run_towboat(config)
}
