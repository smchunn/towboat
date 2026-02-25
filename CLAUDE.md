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

Towboat is a cross-platform dotfile manager with build tags and template variables. All files go through a resolution pipeline (tag processing + template substitution) into `.towboat/resolved/`, and symlinks always point to resolved files — never directly to source.

```
source → resolve (tags + templates) → .towboat/resolved/ → symlink → target
```

## Common Development Commands

- **Build**: `cargo build`
- **Run**: `cargo run -- --help`
- **Test**: `cargo test` (101 tests: unit + integration)
- **Check**: `cargo check`
- **Format**: `cargo fmt`
- **Lint**: `cargo clippy`

## CLI Usage

Towboat uses a subcommand-based interface:

```bash
# Scaffold a new manifest
towboat init

# Sync all packages (resolve + symlink)
towboat sync

# Sync a specific package
towboat sync bash

# Dry run
towboat sync --dry-run

# Force overwrite conflicts
towboat sync --force

# Check file states
towboat status

# Preview changes
towboat diff
```

### Global Options

- `-d, --dir <DIR>`: Stow directory containing packages and `towboat.toml` (default: `.`)
- `-t, --target <DIR>`: Target directory for symlinks (default: `~`)

## Configuration

### System Manifest (`towboat.toml`)

Lives at the stow directory root. Declares active tags, template variables, and packages:

```toml
[system]
tags = ["macos", "laptop", "work"]

[variables]
hostname = "macbook-pro"
email = "user@work.com"

[packages]
bash = {}
vim = { tags = ["development"] }
ssh = { tags = ["work"] }
```

### Package Config (`boat.toml`)

Per-package configuration in each package directory:

```toml
[targets]
".bashrc" = { tags = "linux & laptop" }       # Tag expression
".profile" = { tags = ["linux", "macos"] }     # Tag list (ORed)
"dev-profile.sh" = { target = "profile.sh", tags = ["development"] }
".config/hypr" = { tags = ["linux"] }          # Directory inheritance

[default]
include_all = false
default_tag = "default"
```

### Tag Expressions

Tags support boolean expressions: `"linux & laptop"`, `"macos | default"`, `"!windows"`, `"linux & (laptop | desktop)"`. Precedence: `!` > `&` > `|`.

### In-File Tag Sections

Multiple comment syntaxes supported:

```bash
# {linux-
alias ls='ls --color=auto'
# -linux}

// {macos-
let editor = "code";
// -macos}
```

Supported prefixes: `#`, `//`, `--`, `;`. Open/close must use the same prefix. Tag expressions work in file sections too.

### Template Variables

`${{ var }}` placeholders are substituted from `towboat.toml [variables]`. Undefined variables are hard errors.

```
host = ${{ hostname }}
email = ${{ email }}
```

## Directory Layout

```
dotfiles/                        # Stow directory
├── towboat.toml                 # System manifest
├── bash/
│   ├── boat.toml                # Package config
│   └── .bashrc                  # Source (may have tags + templates)
├── .towboat/                    # Managed by towboat (gitignored)
│   ├── towboat.lock             # Deployment state + hashes
│   └── resolved/
│       └── bash/
│           └── .bashrc          # Resolved: clean, ready-to-use
```

Target (`~`): `~/.bashrc → dotfiles/.towboat/resolved/bash/.bashrc`

## Project Structure

```
src/
├── main.rs              # CLI (clap derive with subcommands)
├── lib.rs               # Public API re-exports
├── error.rs             # TowboatError enum (thiserror)
├── config/
│   ├── manifest.rs      # towboat.toml parsing
│   └── package.rs       # boat.toml parsing
├── tags/
│   ├── mod.rs           # TagExpr enum + evaluate()
│   ├── parser.rs        # In-file multi-syntax tag parsing
│   └── matcher.rs       # Tag expression parsing (recursive descent)
├── template/
│   └── engine.rs        # ${{ var }} substitution
├── resolve/
│   ├── mod.rs           # ResolvedFile, ResolveOutcome types
│   └── resolver.rs      # resolve_file(), resolve_package()
├── deploy/
│   ├── symlink.rs       # Symlink CRUD + state checking
│   └── lock.rs          # Lock file load/save/query
├── discovery/
│   └── walker.rs        # Directory traversal with boat.toml
└── commands/
    ├── sync.rs           # towboat sync
    ├── status.rs         # towboat status
    ├── diff.rs           # towboat diff
    └── init.rs           # towboat init
```

## Architecture

- **Resolution Pipeline**: Every file is resolved through: tag processing → template substitution → write to `.towboat/resolved/`. Files without tags/templates pass through unchanged.
- **Symlinks**: Always point to `.towboat/resolved/` files, never to source. This provides a uniform deployment model.
- **Lock File** (`towboat.lock`): Tracks two hashes per file (source + resolved) enabling three-way drift detection:
  - Source changed, resolved didn't → safe to re-resolve
  - Resolved changed, source didn't → user drift (preserved)
  - Both changed → conflict (requires `--force`)
- **Error Handling**: Library uses `thiserror` (`TowboatError`), commands use `anyhow::Result` with context. Conflicts are non-fatal and collected.

## Testing

101 tests total:
- **Unit tests** (86): Tag expression parsing, in-file tag processing, template engine, config parsing, lock file state, symlink operations, directory discovery
- **Integration tests** (15): Full sync workflow, dry-run, force mode, source change detection, idempotency, CLI subcommands

## Dependencies

- `clap` (derive) — CLI argument parsing
- `thiserror` — typed library errors
- `anyhow` — command-level error context
- `serde` + `toml` — config serialization
- `walkdir` — directory traversal
- `sha2` + `hex` — content hashing
- `chrono` — lock file timestamps
- `regex` — (retained, minimal usage)