# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Towboat is a stow-like CLI tool for managing cross-platform dotfiles with build tags. It allows users to maintain a single set of dotfiles with platform-specific sections using build tags, and selectively deploy them based on the target platform.

## Common Development Commands

- **Build**: `cargo build`
- **Run**: `cargo run -- --help` (see CLI usage)
- **Test**: `cargo test` (runs unit, integration, and fixture tests)
- **Test specific module**: `cargo test integration_tests` or `cargo test test_fixtures`
- **Check (fast compile check)**: `cargo check`
- **Format code**: `cargo fmt`
- **Lint**: `cargo clippy`

## CLI Usage

```bash
# Basic usage
towboat -s /path/to/dotfiles -b linux -t ~

# Dry run to see what would happen
towboat -s /path/to/dotfiles -b macos --dry-run

# Target specific directory
towboat -s ./dotfiles -b windows -t /target/dir
```

## Build Tag Format

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

## Project Structure

- `src/main.rs` - CLI entry point and argument parsing
- `src/lib.rs` - Core library functions (public API)
- `tests/integration_tests.rs` - End-to-end CLI testing
- `tests/test_fixtures.rs` - Test utilities and complex scenario testing
- `tests/fixtures/` - Sample dotfiles for testing
- `Cargo.toml` - Dependencies: clap, anyhow, walkdir, regex + test dependencies

## Architecture

- **CLI Interface**: Built with clap for argument parsing
- **File Discovery**: Uses walkdir to recursively find matching files
- **Build Tag Processing**: Regex-based parsing of build tag sections
- **Symlink Management**: Cross-platform symlink creation with file processing for tagged content
- **File Matching**: Files are included if they contain the build tag in filename (e.g., `.linux`) or have build tag sections in content

## Testing

The project has comprehensive test coverage:

- **Unit Tests** (`src/lib.rs`): Test individual functions like build tag parsing, file discovery, and symlink creation
- **Integration Tests** (`tests/integration_tests.rs`): Test complete CLI workflows including dry-run mode, error handling, and file processing
- **Fixture Tests** (`tests/test_fixtures.rs`): Test complex scenarios with realistic dotfile structures using sample fixtures

Test fixtures include:
- Cross-platform shell configurations (.bashrc variations)
- Git config with platform-specific sections
- Neovim configurations for different platforms
- SSH config with build tags
- Application config files (TOML format)