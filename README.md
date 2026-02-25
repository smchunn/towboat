# Towboat

A cross-platform dotfile manager with build tags and template variables. Every file goes through a resolution pipeline (tag processing + template substitution) into `.towboat/resolved/`, and symlinks always point to resolved files.

## Features

- **Build Tags** with boolean expressions: `"linux & laptop"`, `"macos | default"`, `"!windows"`
- **Template Variables**: `${{ hostname }}`, `${{ email }}` — substituted from manifest
- **Multiple Comment Syntaxes**: `#`, `//`, `--`, `;` for in-file tag sections
- **Three-way Drift Detection**: knows when source changed, resolved was edited, or both
- **Subcommand CLI**: `sync`, `status`, `diff`, `init`

## Installation

```bash
cargo install --path .
```

## Quick Start

1. **Initialize** in your dotfiles directory:

```bash
cd ~/dotfiles
towboat init
```

2. **Edit `towboat.toml`** — inline config (simplest for single-package repos):

```toml
[system]
tags = ["macos", "laptop", "work"]

[variables]
hostname = "macbook-pro"
email = "user@work.com"

[packages.home]
target_dir = "~"

[packages.home.targets]
".bashrc" = { tags = "linux | macos" }
".config/nvim" = { tags = ["macos", "linux"] }
```

Or use separate `boat.toml` files (better for multi-package setups):

```toml
[system]
tags = ["macos", "laptop", "work"]

[variables]
hostname = "macbook-pro"
email = "user@work.com"

[packages]
bash = {}
git = {}
vim = { tags = ["development"] }
```

```bash
mkdir bash
cat > bash/boat.toml << 'EOF'
[targets]
".bashrc" = { tags = "linux | macos" }
EOF
```

3. **Add dotfiles** with tags and templates:

```bash
cat > bash/.bashrc << 'EOF'
export PATH=$PATH:/usr/local/bin

# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}

export HOSTNAME="${{ hostname }}"
EOF
```

4. **Deploy**:

```bash
towboat sync              # Sync all packages
towboat sync bash          # Sync just one
towboat sync --dry-run     # Preview changes
towboat status             # Check file states
towboat diff               # Show pending changes
```

## How It Works

```
source → resolve (tags + templates) → .towboat/resolved/ → symlink → target
```

1. `towboat sync` reads `towboat.toml` for active tags and variables
2. For each package, discovers files matching active tags (via inline config or `boat.toml`)
3. Resolves each file: strips non-matching tag sections, substitutes `${{ variables }}`
4. Writes resolved files to `.towboat/resolved/<package>/`
5. Creates symlinks from target (e.g. `~/.bashrc`) to resolved files
6. Updates `towboat.lock` with source + resolved hashes for drift detection

## Configuration

### `towboat.toml` (System Manifest)

```toml
[system]
tags = ["macos", "laptop", "work"]    # Active tags for this system

[variables]
hostname = "macbook-pro"               # Available as ${{ hostname }}
email = "user@work.com"                # Available as ${{ email }}

[packages]
bash = {}                              # Deploy with system tags
vim = { tags = ["development"] }       # Extra tag requirement
```

### Package Config

Package configuration can live in one of two places — pick one per package:

#### Option A: Inline in `towboat.toml` (recommended for simple repos)

```toml
[packages.home]
target_dir = "~"

[packages.home.targets]
".bashrc" = { tags = "linux & laptop" }
".config/nvim" = { tags = ["macos", "linux"] }

[packages.home.default]
include_all = true
default_tag = "default"
```

#### Option B: Separate `boat.toml` (better for multi-package setups)

```toml
# bash/boat.toml
[targets]
".bashrc" = { tags = "linux & laptop" }           # Boolean expression
".profile" = { tags = ["linux", "macos"] }         # List (ORed together)
"dev-profile.sh" = { target = "profile.sh", tags = ["dev"] }  # Path remap
".config/hypr" = { tags = ["linux"] }              # Directory (all files inherit)

[default]
include_all = true         # Include unconfigured files
default_tag = "default"    # Tag for unconfigured files
```

#### Precedence

| Inline config? | `boat.toml` exists? | Behavior |
|---|---|---|
| Yes | No | Use inline |
| No | Yes | Use `boat.toml` |
| No | No | Default (`include_all: true`) |
| Yes | Yes | **Error** — pick one |

### Tag Expressions

| Expression | Meaning |
|---|---|
| `"linux"` | Single tag |
| `"linux & laptop"` | Both required |
| `"macos \| default"` | Either matches |
| `"!windows"` | Negation |
| `"linux & (laptop \| desktop)"` | Grouped |

Precedence: `!` > `&` > `|`

### In-File Tag Sections

```bash
# {linux-                    # Shell/YAML/Python/TOML
alias ls='ls --color=auto'
# -linux}

// {macos-                   # JS/Rust/C/Go
let editor = "code";
// -macos}

-- {linux & laptop-          # Lua/SQL/Haskell (expressions work too)
local config = "laptop"
-- -linux & laptop}

; {windows-                  # INI/assembly
path = C:\Users
; -windows}
```

Open and close markers must use the same comment prefix.

### Template Variables

```
host = ${{ hostname }}
email = ${{ email }}
```

Undefined variables are hard errors. Escape with `\${{`.

## Drift Detection

The lock file tracks two hashes per file:

| Source changed | Resolved changed | State | Action |
|---|---|---|---|
| No | No | Up to date | Skip |
| Yes | No | Source changed | Re-resolve |
| No | Yes | Drifted | Preserve edits |
| Yes | Yes | Conflict | Error (use `--force`) |

## License

MIT
