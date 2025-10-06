# CLAUDE.md

This document defines the rules and expectations for how you should approach coding tasks in this repository. Follow these guidelines at all times to ensure correctness, maintainability, and user satisfaction.

## General Principles

### Read Entire Files
Never skim. Always read full files before making changes. Otherwise, you risk duplicating code, misunderstanding the architecture, or breaking existing functionality.

### Commit Early and Often
Break large tasks into logical milestones. After each milestone is completed and confirmed by the user, commit it. Small, frequent commits protect progress and make it easier to recover if later steps go wrong.

### Plan Before Coding
For every new task:
- Understand the current architecture
- Identify which files need modification
- Draft a written Plan that includes architectural considerations, edge cases, and a step-by-step approach
- Get the Plan approved by the user before writing any code

### Clarity Over Assumptions
If you are unclear about the task, ask the user questions instead of making assumptions.

### Avoid Unnecessary Refactors
Do not perform large refactors unless explicitly instructed. Small opportunistic cleanups (variable renaming, helper extraction) are fine, but major restructuring requires user approval.

## Libraries & Dependencies

### Stay Up to Date
Your internal knowledge may be outdated. Unless the library interface is extremely stable and you are 100% sure, always confirm the latest syntax and usage via Perplexity (preferred) or web search (fallback).

### Never Skip Libraries
Do not abandon or skip a requested library by claiming "it isn't working." It usually means the syntax or usage is wrong. If the user requested a library, use it.

### Handling Deprecation
If a library is truly deprecated or unsupported, provide evidence (e.g., documentation or release notes) and propose alternatives. Never silently switch libraries.

## Coding Practices

### Linting & Validation
Always run linting and format checks after major changes. This catches syntax errors, incorrect usage, and structural issues before the code is shared.

### Organization & Style
- Separate code into files where appropriate
- Use clear, consistent variable naming
- Keep functions modular and manageable
- Avoid oversized files and functions
- Write concise, meaningful comments

### Readability First
Code is read more often than it is written. Optimize for readability and maintainability above all.

### No Dummy Implementations
Unless explicitly asked, never provide "placeholder" or "this is how it would look" code. Always implement real, working solutions.

## Problem-Solving Mindset

### Root Cause Over Guesswork
If you encounter repeated issues, investigate the root cause. Do not guess randomly or "throw things at the wall."

### Break Down Large Tasks
If a task feels too big or vague, break it into smaller subtasks. If it's still unclear, push back to the user and ask them to help refine or restructure the request.

## UI & UX Work

### Design Standards
When working on UI/UX, ensure your work is:
- Aesthetically pleasing
- Easy to use
- Consistent with established patterns

### Best Practices
- Follow interaction and micro-interaction standards
- Prioritize smooth, engaging, user-friendly flows
- Ensure accessibility (contrast, keyboard navigation, ARIA where relevant)

## Final Principle
Above all, prioritize clarity, correctness, and maintainability. Your goal is to deliver code that future developers (including yourself) can understand and build upon with confidence.

---

## Project Overview

Towboat is a stow-like CLI tool for managing cross-platform dotfiles with build tags. It allows users to maintain a single set of dotfiles with platform-specific sections using build tags, and selectively deploy them based on custom tags (no hardcoded OS detection).

The tool supports two configuration methods:
1. **`boat.toml` files (required)**: TOML configuration files that specify which files and directories should be included for each build tag
2. **Build tag sections in files**: Tag-specific content within files using `# {tag-...# -tag}` syntax

**Important:** Build tags are user-defined. If no build tag is specified via `-b`, the tool defaults to using `"default"` as the build tag. There is no automatic OS detection - all tags are explicit.

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
# Basic usage - symlinks the 'bash' package using "default" build tag
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
towboat -d ./dotfiles -t ~ -b production --dry-run git
```

### Arguments

- `<package>`: Package directory name to symlink (e.g., 'bash', 'vim', 'git')
- `-d, --dir <DIR>`: Stow directory containing packages (default: current directory)
- `-t, --target <DIR>`: Target directory to create symlinks in (default: ~)
- `-b, --build <TAG>`: Build tag to match (defaults to "default" if not specified)
- `--dry-run`: Show what would be done without making changes

## Configuration Methods

### 1. boat.toml Configuration (Required)

Create a `boat.toml` file in your package directory using TOML format:

```toml
# Package-level configuration
target_dir = "~"  # Override target directory for this package
build_tags = ["production"]  # Default build tags for this package

[targets]
# Unified configuration for both files and directories
# Map of source paths to their target paths and build tags
".bashrc" = { tags = ["production", "development"] }  # target defaults to ".bashrc"
".vimrc" = { target = ".vimrc", tags = ["production"] }
"dev-profile.sh" = { target = "profile.sh", tags = ["development"] }
"scripts" = { tags = ["production", "development"] }  # Directory
"bin" = { tags = ["production"] }  # Directory

[default]
# Default behavior for files/directories not explicitly configured
include_all = false  # Set to true to include all files by default
default_tag = "default"  # Tag to assign to untagged files when include_all is true
```

#### Package-level Configuration

- `target_dir`: Override the target directory for this specific package (supports `~` expansion)
- `build_tags`: Default build tags for this package (used when `-b` is not specified; defaults to "default" if omitted)
- `targets`: Unified section for files and directories
  - **Files**: Directly symlinked or processed if they contain build tags
  - **Directories**: All files within inherit the directory's tags (recursive tag inheritance)
- `default.default_tag`: Tag to assign to files/directories not explicitly configured when `include_all` is true (defaults to "default")

#### Directory Tag Inheritance

When you configure a directory in `[targets]`, all files within that directory (recursively) inherit those tags:

```toml
[targets]
".config/hypr" = { tags = ["linux"] }
```

This will include ALL files inside `.config/hypr/` (like `hyprland.conf`, `scripts/startup.sh`, etc.) when deploying with `-b linux`. Individual files within the directory can still be explicitly configured to override the inherited tags.

#### Nested boat.toml Files

If a subdirectory contains its own `boat.toml` file, that configuration takes precedence for that directory and its contents. This allows for fine-grained control over nested directory structures.

#### Handling Existing Files/Directories

When deploying, towboat will error if a target already exists. Use:
- `--force` to overwrite existing targets
- `--adopt` to copy existing targets back to the package directory

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
  - TOML-based `boat.toml` configuration files using serde for parsing (`BoatConfig` struct)
  - Unified `[targets]` section for both files and directories
  - Nested `boat.toml` support with subdirectory precedence
  - Regex-based parsing of build tag sections within files (`# {tag-...# -tag}`)
- **Cache System**:
  - SHA256-based checksum tracking for processed files
  - Stored in `.towboat/checksums.toml` within the stow directory (e.g., `dotfiles/.towboat/`)
  - Cache is local to each dotfiles repository
  - Detects manual modifications to deployed files
  - Prevents accidental overwrites of user edits
- **File Discovery**: Uses walkdir to recursively find matching files and directories based on configuration
- **Deployment Strategy**:
  - Files with build tag content are processed and written directly to target
  - Files and directories without build tags are symlinked to preserve connection to source
  - Processed files are tracked in cache to detect modifications
  - Existing targets require `--force` or `--adopt` flags

## Testing

The project has comprehensive test coverage:

- **Unit Tests** (`src/lib.rs`): Test individual functions like build tag parsing, target discovery, symlink creation, and boat.toml configuration parsing
- **Integration Tests** (`tests/integration_tests.rs`): Test complete CLI workflows including dry-run mode, error handling, file processing, and nested boat.toml precedence
- **Fixture Tests** (`tests/test_fixtures.rs`): Test complex scenarios with realistic dotfile structures using sample fixtures

Test fixtures include:
- Cross-platform shell configurations (.bashrc variations)
- Git config with platform-specific sections
- Neovim configurations for different platforms
- SSH config with build tags
- Application config files (TOML format)
- boat.toml configuration file examples with unified `[targets]` section