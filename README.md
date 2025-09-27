# Towboat 🚢

A modern, cross-platform alternative to GNU Stow for managing dotfiles with build tags. Towboat allows you to maintain a single set of dotfiles with platform-specific sections and deploy them selectively based on your target environment.

[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

- 🔧 **Cross-platform**: Works on Linux, macOS, and Windows
- 🏷️ **Build Tags**: Include/exclude content based on platform or environment
- 📁 **Flexible Structure**: Support for both filename-based and content-based tagging
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

1. **Organize your dotfiles** with build tags:

```bash
dotfiles/
├── .bashrc.linux           # Linux-specific bash config
├── .bashrc.macos           # macOS-specific bash config
├── .gitconfig              # Shared git config with tags
└── .config/
    └── nvim/
        ├── init.vim.linux  # Linux-specific nvim config
        └── init.vim.macos  # macOS-specific nvim config
```

2. **Deploy for your platform**:

```bash
# Deploy Linux dotfiles
towboat -s ~/dotfiles -b linux -t ~

# Preview changes first
towboat -s ~/dotfiles -b linux --dry-run
```

## Build Tag Syntax

Towboat supports two ways to specify platform-specific content:

### 1. Filename-based Tags

Add the build tag as a suffix to your filename:

```
.bashrc.linux    → .bashrc (on Linux only)
.vimrc.macos     → .vimrc (on macOS only)
config.toml.dev  → config.toml (with 'dev' tag)
```

### 2. Content-based Tags

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
towboat [OPTIONS] --source <DIR> --build <TAG>

Options:
  -s, --source <DIR>  Source directory containing dotfiles
  -t, --target <DIR>  Target directory to create symlinks in [default: .]
  -b, --build <TAG>   Build tag to match (e.g., 'linux', 'macos', 'windows')
      --dry-run       Show what would be done without making changes
  -h, --help          Print help
  -V, --version       Print version
```

### Examples

```bash
# Deploy Linux dotfiles to home directory
towboat -s ~/dotfiles -b linux -t ~

# Deploy development configuration to current directory
towboat -s ./config -b dev

# Preview Windows deployment
towboat -s ~/dotfiles -b windows --dry-run

# Deploy to specific target directory
towboat -s ~/dotfiles -b macos -t /tmp/test-deploy
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

1. **File Discovery**: Towboat recursively scans your source directory for files matching the specified build tag
2. **Tag Processing**: Files are included if they:
   - Have the build tag in their filename (e.g., `.bashrc.linux`)
   - Contain build tag sections in their content (e.g., `# {linux-...# -linux}`)
3. **Content Processing**: For files with build tags in content:
   - Matching tag sections are extracted and included
   - Non-matching tag sections are removed
   - Common content outside tags is preserved
4. **Deployment**:
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
├── main.rs          # CLI entry point
└── lib.rs           # Core library functions

tests/
├── integration_tests.rs  # End-to-end CLI testing
├── test_fixtures.rs      # Complex scenario testing
└── fixtures/             # Sample dotfiles for testing
    ├── bashrc_linux.txt
    ├── bashrc_macos.txt
    ├── gitconfig_with_tags.txt
    └── ...
```

## Comparison with GNU Stow

| Feature | GNU Stow | Towboat |
|---------|----------|---------|
| Cross-platform | Unix-like only | Linux, macOS, Windows |
| Platform-specific content | No | Yes (build tags) |
| File processing | Symlinks only | Symlinks + content processing |
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