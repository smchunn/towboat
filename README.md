# Towboat 🚢

A modern, cross-platform alternative to GNU Stow for managing dotfiles with build tags. Towboat allows you to maintain a single set of dotfiles with platform-specific sections and deploy them selectively based on your target environment.

[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

- 🔧 **Cross-platform**: Works on Linux, macOS, and Windows
- 🏷️ **Build Tags**: Include/exclude content based on platform or environment
- 📁 **Flexible Structure**: Support for TOML configuration, filename-based, and content-based tagging
- ⚙️ **boat.toml Configuration**: Explicit control over which files are deployed per build tag
- 🔗 **Smart Linking**: Creates symlinks for unchanged files, processes tagged content
- 🔍 **Dry Run Mode**: Preview changes before applying them
- 🧪 **Well Tested**: Comprehensive unit and integration tests

## Installation

### From Source

```bash
git clone <repository-url>
cd towboat
cargo build --release
sudo cp target/release/towboat /usr/local/bin/
```

### Using Cargo

```bash
cargo install --path .
```

## Quick Start

1. **Organize your dotfiles** in a stow-like package structure:

```bash
dotfiles/
├── bash/                   # Package: bash configuration
│   ├── boat.toml          # Configuration for this package
│   ├── .bashrc            # Bash configuration
│   └── .bash_profile      # Additional bash file
├── vim/                   # Package: vim configuration
│   ├── boat.toml          # Configuration for this package
│   └── .vimrc             # Vim configuration
└── git/                   # Package: git configuration
    ├── boat.toml          # Configuration for this package
    └── .gitconfig         # Git configuration
```

2. **Deploy packages for your platform**:

```bash
# Deploy bash package (auto-detects platform)
towboat bash

# Deploy from specific directory
towboat -d ~/dotfiles bash

# Deploy with specific build tag
towboat -d ~/dotfiles -b linux vim

# Preview changes first
towboat -d ~/dotfiles --dry-run git
```

## Configuration Methods

Towboat supports three ways to specify platform-specific content, with `boat.toml` being the recommended approach:

### 1. boat.toml Configuration (Recommended)

Create a `boat.toml` file in each package directory to explicitly control which files and directories are deployed for each build tag. This provides the most control and clarity.

**Example boat.toml:**

```toml
# Package-level configuration
target_dir = "~"                    # Override target directory (supports ~ expansion)
build_tags = ["linux", "macos"]     # Default build tags for auto-detection

[files]
# Map actual filenames to target paths and build tags
".bashrc" = { target = ".bashrc", tags = ["linux", "macos"] }
".bash_profile" = { target = ".bash_profile", tags = ["macos"] }
"windows-profile.ps1" = { target = "profile.ps1", tags = ["windows"] }

[directories]
# Specify which directories to include per build tag
"scripts" = { tags = ["linux", "macos"] }
"bin" = { tags = ["linux"] }

[default]
# Default behavior for files not explicitly configured
include_all = false  # Set to true to include all unconfigured files
```

**Configuration Options:**

- **Package-level settings:**
  - `target_dir`: Override the target directory for this package (supports `~` and `~/path` expansion)
  - `build_tags`: Default build tags used when `-b` flag is not specified (falls back to platform auto-detection)

- **Files section:**
  - Map source filenames to their deployment configuration
  - `target`: The filename to create in the target directory
  - `tags`: Array of build tags that should include this file

- **Directories section:**
  - Specify which directories should be included per build tag
  - `tags`: Array of build tags that should include this directory

- **Default section:**
  - `include_all`: If `true`, files/directories not explicitly configured are included for all build tags

**Example boat.toml for a bash package:**

```toml
target_dir = "~"
build_tags = ["linux"]

[files]
".bashrc" = { target = ".bashrc", tags = ["linux", "macos"] }
".bash_profile" = { target = ".bash_profile", tags = ["macos"] }
".bash_aliases" = { target = ".bash_aliases", tags = ["linux", "macos", "windows"] }

[directories]
"scripts" = { tags = ["linux", "macos"] }
```

### 2. Filename-based Tags (Legacy)

Add the build tag as a suffix to your filename:

```
.bashrc.linux    → .bashrc (on Linux only)
.vimrc.macos     → .vimrc (on macOS only)
config.toml.dev  → config.toml (with 'dev' tag)
```

### 3. Content-based Tags

Include platform-specific sections within files:

```bash
# Shared configuration
export PATH=$PATH:/usr/local/bin

# {linux-
alias ls='ls --color=auto'
export EDITOR=vim
# -linux}

# {macos-
alias ls='ls -G'
export EDITOR=nano
# -macos}

# {windows-
alias ls='dir'
set EDITOR=notepad
# -windows}

# More shared configuration
echo "Configuration loaded"
```

When deployed with `-b linux`, only the Linux section is included:

```bash
# Shared configuration
export PATH=$PATH:/usr/local/bin

alias ls='ls --color=auto'
export EDITOR=vim

# More shared configuration
echo "Configuration loaded"
```

## Usage

```
towboat [OPTIONS] <PACKAGE>

Arguments:
  <PACKAGE>           Package directory name to symlink (e.g., 'bash', 'vim', 'git')

Options:
  -d, --dir <DIR>     Stow directory containing packages [default: .]
  -t, --target <DIR>  Target directory to create symlinks in [default: ~]
  -b, --build <TAG>   Build tag to match (e.g., 'linux', 'macos', 'windows')
                      Auto-detected from platform if not specified
      --dry-run       Show what would be done without making changes
  -h, --help          Print help
  -V, --version       Print version
```

### Examples

```bash
# Deploy bash package (auto-detects platform)
towboat bash

# Deploy from specific stow directory
towboat -d ~/dotfiles bash

# Deploy with explicit build tag
towboat -d ~/dotfiles -b linux vim

# Preview deployment without making changes
towboat -d ~/dotfiles --dry-run git

# Deploy to custom target directory
towboat -d ~/dotfiles -t /tmp/test bash

# Full example with all options
towboat -d ~/dotfiles -t ~ -b macos --dry-run vim
```

## Real-world Examples

### Multi-platform Shell Configuration

```bash
# .bashrc with platform-specific content
# Common aliases
alias ll='ls -la'
alias ..='cd ..'

# {linux-
alias open='xdg-open'
alias pbcopy='xclip -selection clipboard'
alias pbpaste='xclip -selection clipboard -o'
export BROWSER=firefox
# -linux}

# {macos-
alias cask='brew cask'
export BROWSER=safari
# -macos}

# {windows-
alias open='start'
# -windows}

# Common functions
function mkcd() {
    mkdir -p "$1" && cd "$1"
}
```

### Git Configuration

```ini
# .gitconfig
[user]
    name = Your Name
    email = your.email@example.com

# {linux-
[core]
    editor = vim
    pager = less
# -linux}

# {macos-
[core]
    editor = code --wait
    pager = less
# -macos}

# {windows-
[core]
    editor = notepad
    autocrlf = true
# -windows}

[push]
    default = simple
```

### SSH Configuration

```
# .ssh/config
Host *
    ServerAliveInterval 60
    ServerAliveCountMax 3

# {linux-
Host production
    HostName prod.company.com
    User deploy
    IdentityFile ~/.ssh/id_rsa_prod
    Port 22
# -linux}

# {macos-
Host production
    HostName prod.company.com
    User deploy
    IdentityFile ~/.ssh/id_rsa_prod
    Port 22
    UseKeychain yes
    AddKeysToAgent yes
# -macos}
```

## How It Works

1. **Package Selection**: Towboat uses a stow-like interface where you specify a package name (e.g., `bash`, `vim`)
2. **Configuration Loading**: If a `boat.toml` exists in the package directory, it's used to determine which files to deploy
3. **Build Tag Resolution**: Build tag is determined from:
   - CLI argument (`-b` flag) - highest priority
   - `boat.toml` configuration (`build_tags` array) - medium priority
   - Platform auto-detection (Linux/macOS/Windows) - fallback
4. **File Discovery**: Files are included based on:
   - **boat.toml** configuration (explicit file/directory mappings)
   - **Filename-based tags** (e.g., `.bashrc.linux`)
   - **Content-based tags** (e.g., `# {linux-...# -linux}`)
5. **Content Processing**: For files with build tag sections:
   - Matching tag sections are extracted and included
   - Non-matching tag sections are removed
   - Common content outside tags is preserved
6. **Deployment**:
   - Files with processed content are written directly to the target
   - Files without build tags are symlinked to preserve the connection to source

## Development

### Building

```bash
cargo build          # Debug build
cargo build --release # Release build
```

### Testing

```bash
cargo test                    # Run all tests
cargo test integration_tests # Run integration tests only
cargo test test_fixtures     # Run fixture tests only
```

### Project Structure

```
src/
├── main.rs          # CLI entry point with stow-like interface
└── lib.rs           # Core library functions and boat.toml parsing

tests/
├── integration_tests.rs  # End-to-end CLI testing
├── test_fixtures.rs      # Complex scenario testing
└── fixtures/             # Sample dotfiles for testing
    ├── bashrc_linux.txt
    ├── bashrc_macos.txt
    ├── gitconfig_with_tags.txt
    ├── boat_config.toml
    └── ...
```

## Comparison with GNU Stow

| Feature | GNU Stow | Towboat |
|---------|----------|---------|
| Cross-platform | Unix-like only | Linux, macOS, Windows |
| Package-based deployment | Yes | Yes |
| Platform-specific content | No | Yes (build tags) |
| File processing | Symlinks only | Symlinks + content processing |
| Configuration files | No | Yes (boat.toml) |
| Modern CLI | Basic | Clap-based with help/validation |
| Dry run mode | Yes | Yes |
| Nested structures | Yes | Yes |

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass (`cargo test`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by GNU Stow for the symlink management concept
- Built with the excellent Rust ecosystem: clap, anyhow, walkdir, regex