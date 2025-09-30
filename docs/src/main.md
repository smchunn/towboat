# main.rs - CLI Entry Point

This file handles command-line argument parsing and orchestrates the configuration setup before calling the core library.

## Imports

```rust
use anyhow::Result;
```
- **anyhow::Result**: Provides convenient error handling with context. This is a type alias for `Result<T, anyhow::Error>`, which allows us to return errors from `main()` that will be properly displayed to the user.

```rust
use clap::{Arg, Command};
```
- **clap**: Command-line argument parser library. We use the builder pattern (`Command` and `Arg`) to define our CLI interface.

```rust
use std::path::PathBuf;
```
- **PathBuf**: An owned, mutable path type that works across platforms (handles `/` on Unix and `\` on Windows).

```rust
use towboat::{Config, run_towboat, find_boat_config, parse_boat_config};
```
- **Config**: Struct that holds all configuration needed for deployment
- **run_towboat**: Main library function that performs the actual deployment
- **find_boat_config**: Searches for `boat.toml` configuration files
- **parse_boat_config**: Parses TOML configuration into structured data

## Main Function

```rust
fn main() -> Result<()> {
```
Returns `Result<()>` so errors propagate to the shell with proper exit codes and error messages.

### Command-Line Argument Parsing (Lines 7-46)

```rust
let matches = Command::new("towboat")
    .about("A stow-like tool for cross-platform dotfiles with build tags")
    .version("0.1.0")
```
Creates a new CLI command named "towboat" with description and version metadata.

#### Package Argument (Lines 10-15)

```rust
.arg(
    Arg::new("package")
        .help("Package directory to symlink (e.g., 'bash', 'vim', 'git')")
        .required(true)
        .index(1),
)
```
- **First positional argument**: The package name to deploy (e.g., `towboat bash`)
- **required(true)**: Program cannot run without this
- **index(1)**: This is the first positional argument (after the program name)

#### Directory Flag (Lines 16-23)

```rust
.arg(
    Arg::new("dir")
        .short('d')
        .long("dir")
        .value_name("DIR")
        .help("Stow directory containing packages")
        .default_value("."),
)
```
- **-d/--dir**: Specifies the "stow directory" where packages live
- **default_value(".")**: If not provided, uses current directory
- **Purpose**: Allows organizing packages in a different location (e.g., `towboat -d ~/dotfiles bash`)

#### Target Flag (Lines 24-31)

```rust
.arg(
    Arg::new("target")
        .short('t')
        .long("target")
        .value_name("DIR")
        .help("Target directory to create symlinks in")
        .default_value("~"),
)
```
- **-t/--target**: Where to create symlinks (destination directory)
- **default_value("~")**: Defaults to home directory
- **Purpose**: Allows deploying to non-standard locations (e.g., testing in a temp directory)

#### Build Tag Flag (Lines 32-39)

```rust
.arg(
    Arg::new("build")
        .short('b')
        .long("build")
        .value_name("TAG")
        .help("Build tag to match (e.g., 'linux', 'macos', 'windows')")
        .required(false),
)
```
- **-b/--build**: Platform tag to filter files (linux/macos/windows)
- **required(false)**: Optional - will auto-detect if not provided
- **Purpose**: Override auto-detection or use custom tags

#### Dry-Run Flag (Lines 40-45)

```rust
.arg(
    Arg::new("dry-run")
        .long("dry-run")
        .help("Show what would be done without making changes")
        .action(clap::ArgAction::SetTrue),
)
```
- **--dry-run**: Preview mode flag
- **SetTrue**: Boolean flag (presence = true, absence = false)
- **Purpose**: Safety feature to preview changes before applying them

```rust
.get_matches();
```
Parses actual command-line arguments against the defined schema, validates them, and returns matched values.

### Extract Package and Directories (Lines 48-61)

```rust
let package_name = matches.get_one::<String>("package").unwrap();
```
- Gets the package name (safe to unwrap because it's required)
- Example: If user runs `towboat bash`, this extracts "bash"

```rust
let stow_dir = PathBuf::from(matches.get_one::<String>("dir").unwrap());
let source_dir = stow_dir.join(package_name);
```
- **stow_dir**: Where packages are stored (e.g., `/home/user/dotfiles`)
- **source_dir**: Full path to the specific package (e.g., `/home/user/dotfiles/bash`)
- **join()**: Safely concatenates paths using OS-appropriate separators

### Home Directory Expansion (Lines 52-61)

```rust
let target_str = matches.get_one::<String>("target").unwrap();
let target_dir = if target_str == "~" {
    match std::env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => PathBuf::from("."),
    }
} else {
    PathBuf::from(target_str)
};
```
**Purpose**: Expand shell shorthand `~` to actual home directory path

- **If target is exactly "~"**: Read $HOME environment variable
  - **Success**: Use the home directory path
  - **Failure**: Fall back to current directory
- **Otherwise**: Use the path as-is

**Why this matters**: Rust doesn't automatically expand `~` like shells do, so we must handle it manually.

### Configuration Loading (Lines 63-151)

This is the most complex part - it handles the configuration hierarchy:
1. Check for `boat.toml` in package directory
2. Parse configuration if found
3. Apply overrides in priority order: CLI args > boat.toml > platform detection

```rust
let (final_target_dir, build_tag) = if let Some(config_path) = find_boat_config(&source_dir) {
```
**find_boat_config()**: Searches upward from source_dir for `boat.toml` file
- Returns `Some(path)` if found, `None` otherwise

#### If boat.toml Found (Lines 65-134)

```rust
match parse_boat_config(&config_path) {
    Ok(boat_config) => {
```
**parse_boat_config()**: Reads and parses TOML file into structured data

##### Target Directory Resolution (Lines 67-84)

```rust
let config_target_dir = if let Some(config_target) = boat_config.target_dir {
```
Check if `boat.toml` specifies a custom `target_dir`

**Case 1: Exactly "~"** (Lines 69-73)
```rust
if config_target == "~" {
    match std::env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => target_dir.clone(),
    }
}
```
- Expand `~` to home directory
- Fall back to CLI target if $HOME unavailable

**Case 2: Starts with "~/"** (Lines 74-78)
```rust
else if config_target.starts_with("~/") {
    match std::env::var("HOME") {
        Ok(home) => PathBuf::from(home).join(&config_target[2..]),
        Err(_) => PathBuf::from(&config_target[2..]),
    }
}
```
- Handles paths like "~/.config"
- **&config_target[2..]**: Strips off "~/" to get rest of path (e.g., ".config")
- Joins with home directory

**Case 3: Other paths** (Lines 79-81)
```rust
else {
    PathBuf::from(config_target)
}
```
Use absolute or relative path as-is

**Case 4: No target_dir in config** (Lines 82-84)
```rust
} else {
    target_dir.clone()
}
```
Use the CLI-provided target directory

##### Build Tag Resolution (Lines 86-113)

**Priority order**: CLI flag > boat.toml default_tags > platform detection

**Case 1: CLI build tag provided** (Lines 86-87)
```rust
let config_build_tag = if let Some(cli_build_tag) = matches.get_one::<String>("build") {
    cli_build_tag.clone()
}
```
User explicitly specified tag with `-b` flag - this takes highest priority

**Case 2: boat.toml has default build_tags** (Lines 88-101)
```rust
else if let Some(default_tags) = boat_config.build_tags {
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
}
```
- **default_tags**: List of tags from `boat.toml` (e.g., `["linux", "macos"]`)
- **into_iter().next()**: Take first tag from the list
- **unwrap_or_else()**: If list is empty, fall back to platform detection
- **cfg!(target_os = "...")**: Compile-time OS detection
  - Checks what OS the binary was compiled for
  - Not runtime detection - this is set at compile time

**Case 3: No CLI flag, no boat.toml tags** (Lines 102-113)
```rust
else {
    if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "default".to_string()
    }
}
```
Pure platform detection - same logic as above

```rust
(config_target_dir, config_build_tag)
```
Return both resolved values as a tuple

#### If boat.toml Parse Failed (Lines 117-133)

```rust
Err(_) => {
    let build_tag = matches.get_one::<String>("build")
        .map(|s| s.clone())
        .unwrap_or_else(|| {
            // Platform detection...
        });
    (target_dir, build_tag)
}
```
**Scenario**: `boat.toml` exists but is malformed/invalid TOML
**Action**: Ignore it and use CLI defaults
**Rationale**: Fail gracefully rather than crash

#### If No boat.toml Found (Lines 135-151)

```rust
} else {
    let build_tag = matches.get_one::<String>("build")
        .map(|s| s.clone())
        .unwrap_or_else(|| {
            // Platform detection...
        });
    (target_dir, build_tag)
}
```
**Scenario**: No configuration file exists
**Action**: Use CLI arguments and platform detection
**Behavior**: Legacy mode - relies on filename-based build tags

### Create Config and Run (Lines 153-161)

```rust
let config = Config {
    source_dir,
    target_dir: final_target_dir,
    build_tag,
    dry_run: matches.get_flag("dry-run"),
};
```
**Assemble final configuration**:
- **source_dir**: Package directory to deploy from
- **target_dir**: Where to create symlinks (after all resolution logic)
- **build_tag**: Platform tag (after priority resolution)
- **dry_run**: Boolean flag for preview mode

```rust
run_towboat(config)
```
- Call main library function with finalized configuration
- Returns `Result<()>` which propagates to shell as exit code
- **Success**: Exit code 0
- **Error**: Exit code 1 with error message

## Summary of Logic Flow

1. **Parse CLI arguments** using clap
2. **Extract package name** and construct source directory path
3. **Expand home directory** (~) in target path
4. **Search for boat.toml** in package directory
5. **If boat.toml found**:
   - Parse TOML configuration
   - Resolve target directory (config > CLI)
   - Resolve build tag (CLI > config > platform)
6. **If no boat.toml**:
   - Use CLI arguments directly
   - Fall back to platform detection for build tag
7. **Create Config struct** with finalized settings
8. **Execute deployment** via `run_towboat()`

## Key Design Patterns

- **Configuration cascade**: CLI args > boat.toml > defaults
- **Graceful fallbacks**: Always provide working defaults
- **Home directory handling**: Explicit `~` expansion for cross-platform compatibility
- **Error propagation**: Use `Result<()>` to let errors bubble up with context