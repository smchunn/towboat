# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Towboat is a stow-like CLI tool for managing cross-platform dotfiles with build tags. It allows users to maintain a single set of dotfiles with platform-specific sections using build tags, and selectively deploy them based on the target platform.

The tool supports three configuration methods:
1. **`boat.toml` files (recommended)**: TOML configuration files that specify which files and directories should be included for each build tag
2. **Build tag sections in files**: Platform-specific content within files using `# {tag-...# -tag}` syntax
3. **Legacy filename-based matching**: Files with build tag extensions (e.g., `.bashrc.linux`) for backward compatibility

## Common Development Commands

- **Build**: `cargo build`
- **Run**: `cargo run -- --help` (see CLI usage)
- **Test**: `cargo test` (runs unit, integration, and fixture tests)
- **Test specific module**: `cargo test integration_tests` or `cargo test test_fixtures`
- **Check (fast compile check)**: `cargo check`
- **Format code**: `cargo fmt`
- **Lint**: `cargo clippy`

## CLI Usage

The CLI now follows a stow-like interface where you specify a package name as a positional argument:

```bash
# Basic usage - symlinks the 'bash' package using auto-detected build tag
towboat bash

# Specify custom stow directory
towboat -d /path/to/dotfiles bash

# Override target directory
towboat -t ~/custom-target bash

# Specify build tag explicitly
towboat -b linux bash

# Dry run to see what would happen
towboat --dry-run -b macos vim

# Full example with all options
towboat -d ./dotfiles -t ~ -b windows --dry-run git
```

### Arguments

- `<package>`: Package directory name to symlink (e.g., 'bash', 'vim', 'git')
- `-d, --dir <DIR>`: Stow directory containing packages (default: current directory)
- `-t, --target <DIR>`: Target directory to create symlinks in (default: ~)
- `-b, --build <TAG>`: Build tag to match (auto-detected from platform if not specified)
- `--dry-run`: Show what would be done without making changes

## Configuration Methods

### 1. boat.toml Configuration (Recommended)

Create a `boat.toml` file in your package directory using TOML format:

```toml
# Package-level configuration
target_dir = "~"  # Override target directory for this package
build_tags = ["linux", "macos"]  # Default build tags for this package

[files]
# Map of actual filenames to their target paths and build tags
".bashrc" = { target = ".bashrc", tags = ["linux", "macos"] }
".vimrc" = { target = ".vimrc", tags = ["linux"] }
"windows-profile.ps1" = { target = "profile.ps1", tags = ["windows"] }

[directories]
# Map of directory names to their build tags
"scripts" = { tags = ["linux", "macos"] }
"bin" = { tags = ["linux"] }

[default]
# Default behavior for files/directories not explicitly configured
include_all = false  # Set to true to include all files by default
```

#### Package-level Configuration

- `target_dir`: Override the target directory for this specific package (supports `~` expansion)
- `build_tags`: Default build tags for this package (used when `-b` is not specified)

### 2. Build Tag Sections in Files

Files can contain build-specific sections using the syntax:
```
# {build_tag-
content for this build tag
# -build_tag}
```

Example:
```bash
# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}
```

### 3. Legacy Filename-based Matching

For backward compatibility, files with build tag extensions are still supported:
- `.bashrc.linux` → deployed as `.bashrc` on Linux
- `.vimrc.macos` → deployed as `.vimrc` on macOS

## Project Structure

- `src/main.rs` - CLI entry point and argument parsing (stow-like interface)
- `src/lib.rs` - Core library functions (public API)
- `tests/integration_tests.rs` - End-to-end CLI testing
- `tests/test_fixtures.rs` - Test utilities and complex scenario testing
- `tests/fixtures/` - Sample dotfiles for testing
- `Cargo.toml` - Dependencies: clap, anyhow, walkdir, regex, toml, serde + test dependencies

### Package Directory Structure

When using towboat, organize your dotfiles in a stow-like structure:

```
dotfiles/
├── bash/               # Package: bash configuration
│   ├── boat.toml      # Package configuration
│   ├── .bashrc        # Bash configuration file
│   └── .bash_profile  # Additional bash file
├── vim/               # Package: vim configuration
│   ├── boat.toml      # Package configuration
│   ├── .vimrc         # Vim configuration
│   └── .vim/          # Vim directory
└── git/               # Package: git configuration
    ├── boat.toml      # Package configuration
    └── .gitconfig     # Git configuration
```

Then deploy with: `towboat bash`, `towboat vim`, etc.

## Architecture

- **CLI Interface**: Built with clap for argument parsing (`src/main.rs`)
- **Core Library** (`src/lib.rs`): Contains all business logic with public API
- **Configuration System**:
  - Primary: TOML-based `boat.toml` configuration files using serde for parsing (`BoatConfig` struct)
  - Secondary: Regex-based parsing of build tag sections within files (`# {tag-...# -tag}`)
  - Fallback: Legacy filename-based matching for backward compatibility
- **File Discovery**: Uses walkdir to recursively find matching files based on configuration
- **Deployment Strategy**:
  - Files with build tag content are processed and written directly to target
  - Files without build tags are symlinked to preserve connection to source

## Testing

The project has comprehensive test coverage:

- **Unit Tests** (`src/lib.rs`): Test individual functions like build tag parsing, file discovery, symlink creation, and boat.toml configuration parsing
- **Integration Tests** (`tests/integration_tests.rs`): Test complete CLI workflows including dry-run mode, error handling, file processing, and legacy compatibility
- **Fixture Tests** (`tests/test_fixtures.rs`): Test complex scenarios with realistic dotfile structures using sample fixtures

Test fixtures include:
- Cross-platform shell configurations (.bashrc variations)
- Git config with platform-specific sections
- Neovim configurations for different platforms
- SSH config with build tags
- Application config files (TOML format)
- boat.toml configuration file examples