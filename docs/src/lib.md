# lib.rs - Core Library Implementation

This file contains all the business logic for towboat: configuration parsing, file discovery, content processing, and deployment.

## Module Documentation

```rust
//! Towboat - A cross-platform dotfile manager with build tags
//!
//! This crate provides functionality for managing dotfiles across multiple platforms
//! using build tags to include/exclude platform-specific content.
```
**Doc comments** (//!): Describe the module itself, appear in generated documentation

## Imports

```rust
use anyhow::{Context, Result};
```
- **Context**: Trait to add contextual information to errors
- **Result**: Type alias for `Result<T, anyhow::Error>`

```rust
use regex::Regex;
```
- **Regex**: Pattern matching for extracting build tag sections from files

```rust
use serde::{Deserialize, Serialize};
```
- **Deserialize**: Automatically parse data structures from formats like TOML
- **Serialize**: Convert data structures to formats like TOML/JSON

```rust
use std::collections::HashMap;
```
- **HashMap**: Key-value store for configuration lookups (O(1) access time)

```rust
use std::fs;
use std::path::{Path, PathBuf};
```
- **fs**: File system operations (read, write, create directories)
- **Path**: Borrowed path reference (like &str for paths)
- **PathBuf**: Owned path (like String for paths)

```rust
use walkdir::WalkDir;
```
- **WalkDir**: Recursively traverse directory trees

## Configuration Structures

### TargetConfig (Lines 34-44)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetConfig {
    #[serde(default)]
    pub target: Option<String>,
    pub tags: Vec<String>,
}
```

**Purpose**: Defines how a specific file or directory should be deployed

**Attributes**:
- **#[derive(...)]**: Automatically implement traits
  - **Debug**: Pretty printing for debugging
  - **Clone**: Create deep copies
  - **Deserialize**: Parse from TOML
  - **Serialize**: Convert to TOML

**Fields**:
- **target**: Optional override path for where to deploy (defaults to source path)
- **tags**: Which build tags include this file/directory (e.g., ["linux", "macos"])

**Example in boat.toml**:
```toml
[targets]
".bashrc" = { tags = ["linux", "macos"] }  # target defaults to ".bashrc"
"scripts" = { tags = ["production"] }  # Directory
"dev-config.sh" = { target = "config.sh", tags = ["development"] }  # Renamed
```

This unified structure handles both files and directories.

### DefaultConfig (Lines 47-52)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultConfig {
    pub include_all: bool,
}
```

**Purpose**: Define fallback behavior for unconfigured files

**include_all**:
- **true**: Include all files by default (unless explicitly excluded)
- **false**: Exclude all files by default (unless explicitly included)

**Implementation** (Lines 74-78):
```rust
impl Default for DefaultConfig {
    fn default() -> Self {
        Self { include_all: false }
    }
}
```
Default behavior: **exclude** unconfigured files (opt-in approach)

### BoatConfig (Lines 60-75)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoatConfig {
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,

    #[serde(default)]
    pub default: Option<DefaultConfig>,

    #[serde(default)]
    pub target_dir: Option<String>,

    #[serde(default)]
    pub build_tags: Option<Vec<String>>,
}
```

**Purpose**: Complete `boat.toml` configuration structure

**#[serde(default)]**: If field is missing in TOML, use default value
- HashMap → empty
- Option → None

**Fields**:
- **targets**: Unified map of paths → TargetConfig (handles both files and directories)
- **default**: Fallback behavior for unconfigured files/directories
- **target_dir**: Override default target directory (~)
- **build_tags**: Default tags for this package

**Why HashMap?**
- Fast lookups: O(1) to check if file/directory is configured
- Natural key-value structure matches TOML syntax

### Config (Lines 80-91)

```rust
#[derive(Debug)]
pub struct Config {
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
    pub build_tag: String,
    pub dry_run: bool,
}
```

**Purpose**: Runtime configuration after all resolution logic

**Not serialized** (no Deserialize/Serialize):
- This is the final resolved state passed to `run_towboat()`
- Created by main.rs after merging CLI args + boat.toml

**Fields**:
- **source_dir**: Package directory (e.g., ~/dotfiles/bash)
- **target_dir**: Deployment destination (e.g., ~)
- **build_tag**: Active platform tag (e.g., "linux")
- **dry_run**: Preview mode flag

## Core Functions

### process_file_with_build_tags (Lines 93-147)

```rust
pub fn process_file_with_build_tags(content: &str, build_tag: &str) -> Result<String>
```

**Purpose**: Extract platform-specific sections from file content

**Input**: File content with build tag sections:
```bash
# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}
```

**Output** (for "linux" tag):
```bash
alias ls='ls --color=auto'
```

#### Step 1: Build Regex for Target Tag (Lines 133-135)

```rust
let escaped_tag = regex::escape(build_tag);
let tag_pattern = format!(r"(?s)# \{{{}-\s*\n(.*?)\n# -{}\}}", escaped_tag, escaped_tag);
let tag_regex = Regex::new(&tag_pattern)?;
```

**regex::escape()**: Escapes special regex characters in build_tag
- Example: If tag is "c++", becomes "c\\+\\+"
- Prevents treating tag as regex metacharacters

**Pattern breakdown**:
- `(?s)`: DOTALL mode - `.` matches newlines
- `# \{`: Literal "# {" (escaped braces)
- `{}-`: Build tag name followed by "-"
- `\s*\n`: Optional whitespace then newline
- `(.*?)`: **Capture group** - content inside tags (non-greedy)
- `\n# -`: Newline then "# -"
- `{}`: Build tag name again
- `\}`: Literal "}"

**Example match**:
```
# {linux-
content here
# -linux}
```
Captures: "content here"

#### Step 2: Extract Matching Content (Lines 137-140)

```rust
let mut result = content.to_string();
result = tag_regex.replace_all(&result, "$1").to_string();
```

**replace_all()**: Replace all matches with captured group
- **$1**: References first capture group (the content)
- **Effect**: Unwraps the content from tag markers

**Before**:
```
Some text
# {linux-
alias foo=bar
# -linux}
More text
```

**After**:
```
Some text
alias foo=bar
More text
```

#### Step 3: Remove Other Tags (Lines 142-144)

```rust
let other_tags_regex = Regex::new(r"(?s)# \{[^}]+-\s*\n.*?\n# -[^}]+\}")?;
result = other_tags_regex.replace_all(&result, "").to_string();
```

**Pattern breakdown**:
- `# \{[^}]+-`: "# {" + any chars except "}" + "-"
- `[^}]+`: One or more non-"}" characters (matches any tag name)
- Rest is similar to previous pattern

**Effect**: Remove sections for OTHER build tags

**After this step** (continuing example):
```
Some text
alias foo=bar
More text
```
(macos/windows sections removed)

### parse_boat_config (Lines 149-166)

```rust
pub fn parse_boat_config(config_path: &Path) -> Result<BoatConfig>
```

**Purpose**: Read and parse `boat.toml` file

**Implementation**:

```rust
let content = fs::read_to_string(config_path)
    .context(format!("Failed to read boat.toml file: {}", config_path.display()))?;
```
- **read_to_string()**: Read entire file into String
- **.context()**: Add error context if read fails
- **?**: Propagate error if read fails

```rust
let config: BoatConfig = toml::from_str(&content)
    .context(format!("Failed to parse boat.toml file: {}", config_path.display()))?;
```
- **toml::from_str()**: Parse TOML string into BoatConfig struct
- **Serde magic**: Automatically maps TOML keys to struct fields
- **?**: Propagate error if parse fails

**Error handling**: Provides clear context about which file failed and why

### find_boat_config (Lines 168-193)

```rust
pub fn find_boat_config(dir: &Path) -> Option<PathBuf>
```

**Purpose**: Search upward for `boat.toml` file

**Algorithm**: Walk up directory tree until file is found or root is reached

```rust
let mut current = dir;
loop {
    let config_path = current.join("boat.toml");
    if config_path.exists() && config_path.is_file() {
        return Some(config_path);
    }

    match current.parent() {
        Some(parent) => current = parent,
        None => break,
    }
}
None
```

**Step-by-step**:
1. Start at given directory
2. Check if `boat.toml` exists in current directory
3. If found: return path
4. If not found: move to parent directory
5. If no parent (reached root): return None

**Example**: Given `/home/user/dotfiles/bash`
1. Check `/home/user/dotfiles/bash/boat.toml`
2. Check `/home/user/dotfiles/boat.toml`
3. Check `/home/user/boat.toml`
4. Check `/home/boat.toml`
5. Check `/boat.toml`
6. Return None if never found

**Why upward search?**
- Allows project-wide or package-specific configs
- Package-specific config (closer) takes precedence

### should_include_file_with_boat_config (Lines 195-256)

```rust
pub fn should_include_file_with_boat_config(
    file_path: &Path,
    source_dir: &Path,
    build_tag: &str,
    boat_config: &BoatConfig,
) -> Result<(bool, PathBuf)>
```

**Purpose**: Determine if file should be deployed based on boat.toml

**Returns**: Tuple of (should_include, target_path)
- **should_include**: true if file matches build_tag
- **target_path**: Where to deploy file (may differ from source name)

#### Step 1: Get Relative Path (Lines 213-216)

```rust
let relative_path = file_path.strip_prefix(source_dir)
    .context("Failed to get relative path")?;
let filename = relative_path.to_string_lossy().to_string();
```

**strip_prefix()**: Remove source_dir from file_path
- Example: `/home/user/dotfiles/bash/.bashrc` → `.bashrc`

**to_string_lossy()**: Convert path to string
- "Lossy" because non-UTF8 paths are converted with replacement chars

#### Step 2: Check Explicit File Configuration (Lines 218-223)

```rust
if let Some(file_config) = boat_config.files.get(&filename) {
    let should_include = file_config.tags.contains(&build_tag.to_string());
    let target_path = PathBuf::from(&file_config.target);
    return Ok((should_include, target_path));
}
```

**Priority 1**: Explicitly configured files

**Logic**:
- Look up file in files HashMap
- Check if build_tag is in tags list
- Use configured target path (may differ from source)

**Example**:
```toml
[targets]
"linux-bashrc" = { target = ".bashrc", tags = ["linux"] }
```
Source file `linux-bashrc` deploys as `.bashrc` on Linux only.

#### Step 3: Check for Directory Match (Lines 225-232)

Directories can also be configured in the unified `[targets]` section:

**Example**:
```toml
[targets]
"scripts" = { tags = ["linux"] }
```
The entire `scripts/` directory will only be deployed on Linux.

**Note**: When a directory is configured, all files within inherit those tags unless explicitly configured otherwise.

#### Step 4: Check Default Behavior (Lines 234-253)

```rust
let default_fallback = DefaultConfig::default();
let default_config = boat_config.default.as_ref().unwrap_or(&default_fallback);

if default_config.include_all {
```

**Priority 3**: Default include_all setting

**If include_all is true**:

```rust
if file_path.is_file() {
    let content = fs::read_to_string(file_path)
        .context(format!("Failed to read file: {}", file_path.display()))?;

    let escaped_tag = regex::escape(build_tag);
    let tag_pattern = format!(r"# \{{{}-", escaped_tag);
    let tag_regex = Regex::new(&tag_pattern)?;

    if tag_regex.is_match(&content) {
        return Ok((true, relative_path.to_path_buf()));
    }
}
```

**Additional check**: Even with include_all, check file content for build tags
- Read file content
- Search for build tag markers (e.g., `# {linux-`)
- If found: include this file

**Why?** Files with explicit build tags are always relevant for that tag.

```rust
return Ok((true, relative_path.to_path_buf()));
```
**If no build tags found**: Still include (because include_all is true)

**If include_all is false**:
```rust
Ok((false, relative_path.to_path_buf()))
```
Exclude unconfigured files by default.

### should_include_file (Lines 258-294)

```rust
pub fn should_include_file(file_path: &Path, build_tag: &str) -> Result<bool>
```

**Purpose**: Legacy file matching (when no boat.toml exists)

**Two matching strategies**:

#### Strategy 1: Filename Contains Build Tag (Lines 273-278)

```rust
if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
    if filename.contains(&format!(".{}", build_tag)) {
        return Ok(true);
    }
}
```

**Pattern**: `.bashrc.linux`, `.vimrc.macos`, etc.

**Logic**:
- Extract filename from path
- Check if it contains ".{build_tag}"
- Example: `.bashrc.linux` matches "linux" tag

**Limitation**: Simple substring match, not robust
- `.myfile.linux.backup` would match "linux"
- Works for common dotfile naming conventions

#### Strategy 2: Content Contains Build Tags (Lines 280-291)

```rust
if file_path.is_file() {
    let content = fs::read_to_string(file_path)
        .context(format!("Failed to read file: {}", file_path.display()))?;

    let escaped_tag = regex::escape(build_tag);
    let tag_pattern = format!(r"# \{{{}-", escaped_tag);
    let tag_regex = Regex::new(&tag_pattern)?;

    if tag_regex.is_match(&content) {
        return Ok(true);
    }
}
```

**Pattern**: File contains `# {linux-` sections

**Logic**:
- Read entire file
- Search for build tag opening marker
- If found: file contains platform-specific content

**Performance note**: Reads full file content (okay for dotfiles)

```rust
Ok(false)
```
If neither strategy matches, exclude file.

### discover_files_with_boat_config (Lines 296-361)

```rust
pub fn discover_files_with_boat_config(source_dir: &Path, build_tag: &str)
    -> Result<Vec<(PathBuf, PathBuf)>>
```

**Purpose**: Find all files to deploy using boat.toml configuration

**Returns**: Vec of (source_path, target_path) tuples

#### Step 1: Look for boat.toml (Lines 310-336)

```rust
let config_path = match find_boat_config(source_dir) {
    Some(path) => path,
    None => {
        // Fall back to legacy behavior
    }
};
```

**If no boat.toml found**: Fall back to legacy file discovery

```rust
let legacy_files = discover_files(source_dir, build_tag)?;
return Ok(legacy_files.into_iter().map(|p| {
    let relative_path = p.strip_prefix(source_dir).unwrap_or(&p);

    let target_filename = if let Some(filename) = relative_path.file_name().and_then(|n| n.to_str()) {
        let clean_filename = filename.replace(&format!(".{}", build_tag), "");
        if let Some(parent) = relative_path.parent() {
            parent.join(clean_filename)
        } else {
            PathBuf::from(clean_filename)
        }
    } else {
        relative_path.to_path_buf()
    };

    (p.clone(), target_filename)
}).collect());
```

**Legacy processing**:
1. Call `discover_files()` (filename/content matching)
2. For each matched file:
   - Strip source_dir prefix to get relative path
   - Remove build tag from filename (`.bashrc.linux` → `.bashrc`)
   - Create (source, target) tuple

**Example transformation**:
- Source: `/home/user/dotfiles/bash/.bashrc.linux`
- Target: `.bashrc`

#### Step 2: Parse boat.toml (Line 338)

```rust
let boat_config = parse_boat_config(&config_path)?;
```

Read and parse configuration file.

#### Step 3: Walk Directory Tree (Lines 340-358)

```rust
for entry in WalkDir::new(source_dir) {
    let entry = entry.context("Failed to read directory entry")?;
    let path = entry.path();

    if path.file_name() == Some(std::ffi::OsStr::new("boat.toml")) {
        continue;
    }

    if path.is_file() {
        let (should_include, target_path) = should_include_file_with_boat_config(
            path, source_dir, build_tag, &boat_config
        )?;

        if should_include {
            matching_files.push((path.to_path_buf(), target_path));
        }
    }
}
```

**WalkDir::new()**: Recursively traverse directory tree
- Finds all files and subdirectories
- Depth-first traversal

**Skip boat.toml**: Don't deploy configuration file itself

**For each file**:
1. Check if it should be included (via boat.toml rules)
2. Get target path (may be renamed)
3. Add to results if included

**Result**: List of (source, target) tuples ready for deployment

### discover_files (Lines 363-389)

```rust
pub fn discover_files(source_dir: &Path, build_tag: &str) -> Result<Vec<PathBuf>>
```

**Purpose**: Legacy file discovery (simpler, without boat.toml)

**Returns**: Vec of source paths (target paths computed later)

```rust
let mut matching_files = Vec::new();

for entry in WalkDir::new(source_dir) {
    let entry = entry.context("Failed to read directory entry")?;
    let path = entry.path();

    if path.is_file() && should_include_file(path, build_tag)? {
        matching_files.push(path.to_path_buf());
    }
}

Ok(matching_files)
```

**Simple algorithm**:
1. Walk directory tree
2. For each file, check if it matches build tag (filename or content)
3. Add matching files to list

**No configuration**: Purely convention-based matching

### create_symlink_or_file (Lines 391-457)

```rust
pub fn create_symlink_or_file(source: &Path, target: &Path, build_tag: &str, dry_run: bool)
    -> Result<()>
```

**Purpose**: Deploy a file (either symlink or processed copy)

**Two deployment modes**:
1. **Files with build tags**: Process content and write new file
2. **Files without build tags**: Create symlink to preserve source connection

#### Step 1: Create Parent Directories (Lines 407-416)

```rust
if let Some(parent) = target.parent() {
    if !parent.exists() {
        if dry_run {
            println!("Would create directory: {}", parent.display());
        } else {
            fs::create_dir_all(parent)
                .context(format!("Failed to create directory: {}", parent.display()))?;
        }
    }
}
```

**Ensure directory structure exists** before creating file

**create_dir_all()**: Creates directory and all missing parents
- Like `mkdir -p` in shell

**Dry-run mode**: Only print what would be done

#### Step 2: Read and Check Content (Lines 418-424)

```rust
let content = fs::read_to_string(source)
    .context(format!("Failed to read source file: {}", source.display()))?;

let escaped_tag = regex::escape(build_tag);
let tag_pattern = format!(r"# \{{{}-", escaped_tag);
let tag_regex = Regex::new(&tag_pattern)?;
```

**Read source file** to check if it contains build tags

**Create regex** to detect build tag markers

#### Decision: Process or Symlink (Lines 426-454)

**If file has build tags** (Lines 426-436):

```rust
if tag_regex.is_match(&content) {
    let processed_content = process_file_with_build_tags(&content, build_tag)?;

    if dry_run {
        println!("Would create processed file: {} -> {}", source.display(), target.display());
    } else {
        fs::write(target, processed_content)
            .context(format!("Failed to write processed file: {}", target.display()))?;
        println!("Created processed file: {}", target.display());
    }
}
```

**Processing workflow**:
1. Extract platform-specific sections
2. Write processed content to target
3. **Result**: Target is a standalone file with only relevant content

**Why write instead of symlink?**
- Source contains sections for ALL platforms
- Target should only contain sections for THIS platform
- Can't achieve this with symlink

**If file has no build tags** (Lines 437-453):

```rust
else {
    if dry_run {
        println!("Would create symlink: {} -> {}", source.display(), target.display());
    } else {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, target)
                .context(format!("Failed to create symlink: {} -> {}", source.display(), target.display()))?;
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(source, target)
                .context(format!("Failed to create symlink: {} -> {}", source.display(), target.display()))?;
        }

        println!("Created symlink: {}", target.display());
    }
}
```

**Symlinking workflow**:
1. Create symlink pointing to source
2. **Result**: Target is a link to source

**Why symlink?**
- No platform-specific content to process
- Preserves connection to source (changes reflect automatically)
- More efficient (no file duplication)

**Platform-specific symlink APIs**:
- **Unix**: `std::os::unix::fs::symlink()` - general symlink
- **Windows**: `std::os::windows::fs::symlink_file()` - file-specific symlink
  - Windows distinguishes file vs directory symlinks

**#[cfg(...)]**: Conditional compilation
- Only include code for the target platform
- Prevents Windows code from compiling on Unix and vice versa

### run_towboat (Lines 459-532)

```rust
pub fn run_towboat(config: Config) -> Result<()>
```

**Purpose**: Main orchestration function - ties everything together

#### Step 1: Validate Source Directory (Lines 489-491)

```rust
if !config.source_dir.exists() {
    return Err(anyhow::anyhow!("Source directory does not exist: {}", config.source_dir.display()));
}
```

**Early validation**: Fail fast if source doesn't exist

**anyhow::anyhow!()**: Create ad-hoc error with formatted message

#### Step 2: Resolve Target Directory (Lines 493-497)

```rust
let target_dir = if config.target_dir.is_relative() {
    std::env::current_dir()?.join(&config.target_dir)
} else {
    config.target_dir.clone()
};
```

**Handle relative paths**: Convert to absolute paths

**Why?**
- Symlinks work better with absolute paths
- Prevents confusion about what directory we're targeting

**Logic**:
- **Relative path**: Resolve relative to current working directory
- **Absolute path**: Use as-is

#### Step 3: Print Configuration (Lines 499-506)

```rust
println!("Towboat - Cross-platform dotfile manager");
println!("Source: {}", config.source_dir.display());
println!("Target: {}", target_dir.display());
println!("Build tag: {}", config.build_tag);
if config.dry_run {
    println!("DRY RUN - No changes will be made");
}
println!();
```

**User feedback**: Show what's about to happen

**Dry-run warning**: Make it clear when in preview mode

#### Step 4: Discover Files (Line 508)

```rust
let matching_files = discover_files_with_boat_config(&config.source_dir, &config.build_tag)?;
```

**Find all files** to deploy based on configuration

**Result**: Vec of (source, target) path tuples

#### Step 5: Check for Empty Results (Lines 510-513)

```rust
if matching_files.is_empty() {
    println!("No files found matching build tag '{}'", config.build_tag);
    return Ok(());
}
```

**Early exit**: If no files match, there's nothing to do

**Not an error**: Valid scenario (maybe wrong build tag specified)

#### Step 6: Deploy Files (Lines 515-523)

```rust
println!("Found {} matching files:", matching_files.len());

for (source_file, target_relative_path) in &matching_files {
    let target_path = target_dir.join(target_relative_path);

    println!("Processing: {} -> {}", source_file.display(), target_path.display());

    create_symlink_or_file(source_file, &target_path, &config.build_tag, config.dry_run)?;
}
```

**Deployment loop**:
1. Show file count
2. For each (source, target) pair:
   - Construct absolute target path
   - Print what's being processed
   - Deploy file (symlink or process)

**Error propagation**: If any deployment fails, stop immediately (?)

#### Step 7: Print Summary (Lines 525-529)

```rust
if config.dry_run {
    println!("\nDry run completed. Use without --dry-run to apply changes.");
} else {
    println!("\nCompleted successfully!");
}
```

**Final status message**:
- **Dry run**: Remind user how to apply changes
- **Real run**: Confirm success

## Test Suite Overview (Lines 534-748)

The test suite covers all major functionality:

### Test 1: parse_boat_config (Lines 540-580)
- Create temporary boat.toml file
- Parse it into BoatConfig struct
- Verify all fields parsed correctly

### Test 2: should_include_file_with_boat_config (Lines 582-616)
- Create BoatConfig with specific rules
- Test file inclusion for different build tags
- Verify target path resolution

### Test 3: discover_files_with_boat_config (Lines 618-645)
- Create boat.toml and test files
- Discover files for specific build tag
- Verify only matching files found

### Test 4: discover_files_with_boat_config_fallback (Lines 647-663)
- Test legacy behavior (no boat.toml)
- Verify filename-based matching
- Verify build tag removed from target filename

### Test 5-6: process_file_with_build_tags (Lines 665-712)
- Test content processing for Linux and macOS
- Verify correct sections extracted
- Verify other sections removed

### Test 7: should_include_file_by_filename (Lines 714-725)
- Test legacy filename matching
- Verify `.bashrc.linux` matches "linux" tag

### Test 8: discover_files (Lines 727-747)
- Test legacy file discovery
- Verify both filename and content matching work
- Verify non-matching files excluded

## Key Design Patterns

### 1. Configuration Hierarchy
- Explicit (boat.toml) > Convention (filename) > Content (build tags)
- Multiple fallback levels ensure flexibility

### 2. Two Deployment Strategies
- **Symlink**: Fast, preserves source connection
- **Process**: Extracts platform-specific content

### 3. Error Context
- Every error has context about what failed
- `.context()` adds user-friendly messages

### 4. Separation of Concerns
- Discovery: Find files
- Validation: Check if files match
- Processing: Extract content
- Deployment: Create symlinks/files
- Orchestration: run_towboat ties it together

### 5. Graceful Degradation
- boat.toml not found → legacy mode
- Parse error → continue with defaults
- No matches → informative message, not error

## Performance Considerations

1. **File reading**: Only reads files when needed
   - Legacy mode: Reads to check for build tags
   - boat.toml mode: Only reads if include_all is true

2. **Regex compilation**: Compiled once per function call
   - Could be cached for better performance

3. **Directory traversal**: Single-pass with WalkDir
   - Efficient recursive traversal

4. **HashMap lookups**: O(1) configuration checks
   - Fast even with many configured files

## Summary

This library provides a flexible, extensible system for managing cross-platform dotfiles:

- **Configuration**: TOML-based with legacy fallbacks
- **Discovery**: Find files based on multiple criteria
- **Processing**: Extract platform-specific content
- **Deployment**: Smart choice between symlink and file copy
- **Testing**: Comprehensive coverage of all workflows