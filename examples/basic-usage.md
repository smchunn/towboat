# Basic Usage Examples

This document provides practical examples of using towboat for common dotfile management scenarios.

## Getting Started

### 1. Simple Cross-Platform Shell Configuration

Create a `.bashrc` file with platform-specific content:

```bash
# ~/.dotfiles/.bashrc
# Common settings
export EDITOR=vim
export HISTSIZE=10000

# Platform-specific aliases
# {linux-
alias ls='ls --color=auto'
alias ll='ls -alF --color=auto'
alias grep='grep --color=auto'
alias open='xdg-open'
# -linux}

# {macos-
alias ls='ls -G'
alias ll='ls -alF -G'
alias grep='grep --color=auto'
alias open='open'
# -macos}

# {windows-
alias ls='dir'
alias ll='dir /A'
# -windows}

# Common functions
function mkcd() {
    mkdir -p "$1" && cd "$1"
}
```

Deploy to your home directory:

```bash
# For Linux
towboat -s ~/.dotfiles -b linux -t ~

# For macOS
towboat -s ~/.dotfiles -b macos -t ~

# Preview what would happen on Windows
towboat -s ~/.dotfiles -b windows -t ~ --dry-run
```

### 2. Git Configuration with Platform Differences

```ini
# ~/.dotfiles/.gitconfig
[user]
    name = Your Name
    email = your.email@example.com

[core]
    quotepath = false
    autocrlf = input

# {linux-
[core]
    editor = vim
    pager = less
[credential]
    helper = store
# -linux}

# {macos-
[core]
    editor = code --wait
    pager = less
[credential]
    helper = osxkeychain
# -macos}

# {windows-
[core]
    editor = notepad
    autocrlf = true
[credential]
    helper = manager
# -windows}

[push]
    default = simple
[pull]
    rebase = true
```

## Advanced Examples

### 3. Filename-Based Organization

Organize files by platform using filename suffixes:

```
~/.dotfiles/
├── .vimrc.linux          # Linux-specific Vim config
├── .vimrc.macos          # macOS-specific Vim config
├── .tmux.conf.linux      # Linux tmux configuration
├── .tmux.conf.macos      # macOS tmux configuration
└── .gitignore_global     # Shared across all platforms
```

When deployed with `towboat -s ~/.dotfiles -b linux`, the files become:
- `.vimrc.linux` → `.vimrc`
- `.tmux.conf.linux` → `.tmux.conf`
- `.gitignore_global` → symlinked as-is (no platform-specific content)

### 4. Development vs Production Environments

Use build tags for different environments:

```toml
# ~/.dotfiles/config.toml
[app]
name = "myapp"

# {dev-
[database]
host = "localhost"
port = 5432
debug = true

[logging]
level = "debug"
# -dev}

# {prod-
[database]
host = "prod.example.com"
port = 5432
debug = false

[logging]
level = "info"
# -prod}
```

Deploy different configurations:

```bash
# Development environment
towboat -s ~/.dotfiles -b dev -t ./app-config

# Production environment
towboat -s ~/.dotfiles -b prod -t /etc/myapp
```

### 5. SSH Configuration with Host-Specific Settings

```
# ~/.dotfiles/.ssh/config
# Global settings
Host *
    ServerAliveInterval 60
    ServerAliveCountMax 3
    StrictHostKeyChecking ask

# {work-
Host work-server
    HostName server.company.com
    User employee
    IdentityFile ~/.ssh/id_rsa_work
    Port 22

Host work-db
    HostName db.company.com
    User dbadmin
    IdentityFile ~/.ssh/id_rsa_work
    Port 2222
# -work}

# {personal-
Host personal-vps
    HostName myserver.example.com
    User admin
    IdentityFile ~/.ssh/id_rsa_personal
    Port 22
# -personal}

# Common development server
Host devbox
    HostName dev.example.com
    User developer
    IdentityFile ~/.ssh/id_rsa_dev
```

Deploy work or personal configurations:

```bash
# Work setup
towboat -s ~/.dotfiles -b work -t ~

# Personal setup
towboat -s ~/.dotfiles -b personal -t ~
```

## Directory Structure Examples

### Simple Structure
```
dotfiles/
├── .bashrc              # Contains build tags
├── .vimrc.linux        # Linux-specific
├── .vimrc.macos        # macOS-specific
└── .gitconfig          # Contains build tags
```

### Complex Nested Structure
```
dotfiles/
├── .bashrc
├── .gitconfig
├── .config/
│   ├── nvim/
│   │   ├── init.vim.linux
│   │   ├── init.vim.macos
│   │   └── plugins.vim     # Shared
│   ├── tmux/
│   │   ├── tmux.conf
│   │   └── themes/
│   │       ├── linux.conf
│   │       └── macos.conf
│   └── app/
│       └── config.toml    # Contains build tags
├── .ssh/
│   ├── config            # Contains build tags
│   └── known_hosts       # Shared
└── scripts/
    ├── setup.sh.linux
    ├── setup.sh.macos
    └── common.sh          # Shared
```

## Tips and Best Practices

### 1. Use Dry Run First
Always preview changes before applying them:

```bash
towboat -s ~/.dotfiles -b linux --dry-run
```

### 2. Combine Approaches
Mix filename-based and content-based tagging:

```bash
# .bashrc.linux with content tags for fine-grained control
# Common Linux settings
export BROWSER=firefox

# {desktop-
export DISPLAY=:0
alias screenshot='gnome-screenshot'
# -desktop}

# {server-
# No GUI-related settings
alias ll='ls -la'
# -server}
```

Deploy with multiple tags:
```bash
# Desktop Linux
towboat -s ~/.dotfiles -b linux -t ~
# Then overlay desktop-specific settings
towboat -s ~/.dotfiles -b desktop -t ~
```

### 3. Organize by Purpose
```
dotfiles/
├── shell/           # Shell configurations
│   ├── .bashrc
│   ├── .zshrc.macos
│   └── aliases.sh
├── editors/         # Editor configurations
│   ├── .vimrc.linux
│   ├── .vimrc.macos
│   └── nvim/
├── dev/            # Development tools
│   ├── .gitconfig
│   └── tools/
└── system/         # System-specific configs
    ├── linux/
    ├── macos/
    └── windows/
```

### 4. Version Control Integration
```bash
# Initialize git repo for your dotfiles
cd ~/.dotfiles
git init
git add .
git commit -m "Initial dotfiles"

# Deploy and track changes
towboat -s ~/.dotfiles -b $(uname -s | tr '[:upper:]' '[:lower:]') -t ~
```

This approach allows you to version control your dotfiles while maintaining platform-specific variations.