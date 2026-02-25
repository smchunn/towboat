//! Line-by-line state-machine parser for in-file build tag sections.
//!
//! Supports multiple comment syntaxes:
//! - `#`  — shell, YAML, Python, TOML
//! - `//` — JS, Rust, C, Go
//! - `--` — Lua, SQL, Haskell
//! - `;`  — INI, assembly
//!
//! Open/close markers must use the same comment prefix.
//! Tags can now be full boolean expressions (parsed via `tags::matcher`).

use std::collections::HashSet;

use crate::error::{Result, TowboatError};
use crate::tags::matcher;

/// Comment prefixes we recognise as tag delimiters.
const COMMENT_PREFIXES: &[&str] = &["#", "//", "--", ";"];

/// Result of parsing a file's tagged content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    /// The processed output with matching sections kept and non-matching removed.
    pub content: String,
    /// Whether the file contained any tag sections at all.
    pub had_tags: bool,
}

/// Process file content by evaluating build tag sections against active tags.
///
/// Sections are delimited by `<prefix> {<expr>-` (open) and `<prefix> -<expr>}` (close),
/// where `<prefix>` is one of the recognised comment styles.
///
/// Content outside any tag section is always included.
/// Content inside a matching section is included; non-matching sections are stripped.
pub fn process_tags(content: &str, active_tags: &HashSet<String>) -> Result<ParsedFile> {
    let mut output = String::with_capacity(content.len());
    let mut had_tags = false;

    // State: when inside a tag section, track the opening prefix and tag expression.
    let mut current_section: Option<SectionState> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(ref state) = current_section {
            // Check for close marker with matching prefix
            if let Some(expr_str) = try_parse_close(trimmed, &state.prefix) {
                if expr_str != state.tag_expr_str {
                    return Err(TowboatError::MismatchedTagDelimiters {
                        open: state.tag_expr_str.clone(),
                        close: expr_str.to_string(),
                    });
                }
                // End of section — don't output the close marker line
                current_section = None;
                continue;
            }

            // Inside a section: only include content if the tag matches
            if state.matches {
                output.push_str(line);
                output.push('\n');
            }
        } else if let Some((prefix, expr_str)) = try_parse_open(trimmed) {
            // Opening a new tag section
            had_tags = true;
            let tag_expr = matcher::parse(&expr_str)?;
            let matches = tag_expr.evaluate(active_tags);
            current_section = Some(SectionState {
                prefix: prefix.to_string(),
                tag_expr_str: expr_str.to_string(),
                matches,
            });
            // Don't output the open marker line
        } else {
            // Regular line outside any section
            output.push_str(line);
            output.push('\n');
        }
    }

    // Handle unclosed section (treat remaining content as if section was closed)
    if let Some(state) = current_section {
        return Err(TowboatError::MismatchedTagDelimiters {
            open: state.tag_expr_str,
            close: "<EOF>".to_string(),
        });
    }

    // Remove trailing newline if the original didn't end with one
    if !content.ends_with('\n') && output.ends_with('\n') {
        output.pop();
    }

    Ok(ParsedFile {
        content: output,
        had_tags,
    })
}

/// Check if a file contains any tag sections (quick scan without full processing).
pub fn has_tag_sections(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        try_parse_open(trimmed).is_some()
    })
}

struct SectionState {
    prefix: String,
    tag_expr_str: String,
    matches: bool,
}

/// Try to parse an opening tag marker from a trimmed line.
/// Returns `(comment_prefix, tag_expression_string)` on success.
fn try_parse_open(trimmed: &str) -> Option<(&str, String)> {
    for prefix in COMMENT_PREFIXES {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('{')
                && let Some(expr_str) = rest.strip_suffix('-')
            {
                let expr_str = expr_str.trim();
                if !expr_str.is_empty() {
                    return Some((prefix, expr_str.to_string()));
                }
            }
        }
    }
    None
}

/// Try to parse a closing tag marker from a trimmed line.
/// Returns the tag expression string on success (must match opening prefix).
fn try_parse_close(trimmed: &str, expected_prefix: &str) -> Option<String> {
    let rest = trimmed.strip_prefix(expected_prefix)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('-')?;
    let expr_str = rest.strip_suffix('}')?;
    let expr_str = expr_str.trim();
    if !expr_str.is_empty() {
        Some(expr_str.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn shell_comment_tags() {
        let content = "\
# common stuff
export PATH=$PATH:/usr/local/bin

# {linux-
alias ls='ls --color=auto'
# -linux}

# {macos-
alias ls='ls -G'
# -macos}

# more common stuff
";
        let result = process_tags(content, &tags(&["linux"])).unwrap();
        assert!(result.had_tags);
        assert!(result.content.contains("--color=auto"));
        assert!(!result.content.contains("-G"));
        assert!(result.content.contains("common stuff"));
        assert!(result.content.contains("more common stuff"));
    }

    #[test]
    fn c_style_comment_tags() {
        let content = "\
const config = {
// {linux-
  editor: 'vim',
// -linux}
// {macos-
  editor: 'code',
// -macos}
};
";
        let result = process_tags(content, &tags(&["macos"])).unwrap();
        assert!(result.had_tags);
        assert!(result.content.contains("'code'"));
        assert!(!result.content.contains("'vim'"));
    }

    #[test]
    fn lua_style_comment_tags() {
        let content = "\
-- {linux-
local editor = 'vim'
-- -linux}
-- {macos-
local editor = 'code'
-- -macos}
";
        let result = process_tags(content, &tags(&["linux"])).unwrap();
        assert!(result.had_tags);
        assert!(result.content.contains("'vim'"));
        assert!(!result.content.contains("'code'"));
    }

    #[test]
    fn ini_style_comment_tags() {
        let content = "\
[section]
; {linux-
path = /usr/bin
; -linux}
; {windows-
path = C:\\Program Files
; -windows}
";
        let result = process_tags(content, &tags(&["linux"])).unwrap();
        assert!(result.had_tags);
        assert!(result.content.contains("/usr/bin"));
        assert!(!result.content.contains("Program Files"));
    }

    #[test]
    fn boolean_expression_tags() {
        let content = "\
# {linux & laptop-
special laptop linux config
# -linux & laptop}
# {macos | default-
default or mac config
# -macos | default}
";
        let result = process_tags(content, &tags(&["linux", "laptop"])).unwrap();
        assert!(result.content.contains("special laptop linux config"));
        assert!(!result.content.contains("default or mac config"));

        let result2 = process_tags(content, &tags(&["default"])).unwrap();
        assert!(!result2.content.contains("special laptop linux config"));
        assert!(result2.content.contains("default or mac config"));
    }

    #[test]
    fn no_tags_passthrough() {
        let content = "just a normal file\nwith no tags\n";
        let result = process_tags(content, &tags(&["linux"])).unwrap();
        assert!(!result.had_tags);
        assert_eq!(result.content, content);
    }

    #[test]
    fn mismatched_delimiters_error() {
        let content = "\
# {linux-
some content
# -macos}
";
        let result = process_tags(content, &tags(&["linux"]));
        assert!(result.is_err());
    }

    #[test]
    fn mismatched_comment_prefix_not_closed() {
        // Open with # but try to close with // — won't match, so section is unclosed
        let content = "\
# {linux-
some content
// -linux}
";
        let result = process_tags(content, &tags(&["linux"]));
        assert!(result.is_err());
    }

    #[test]
    fn has_tag_sections_detection() {
        assert!(has_tag_sections("# {linux-\nstuff\n# -linux}"));
        assert!(has_tag_sections("// {macos-\nstuff\n// -macos}"));
        assert!(!has_tag_sections("just a normal file"));
    }

    #[test]
    fn empty_content() {
        let result = process_tags("", &tags(&["linux"])).unwrap();
        assert!(!result.had_tags);
        assert_eq!(result.content, "");
    }

    #[test]
    fn negation_tag_in_file() {
        let content = "\
# {!windows-
unix-only config
# -!windows}
";
        let result = process_tags(content, &tags(&["linux"])).unwrap();
        assert!(result.content.contains("unix-only config"));

        let result2 = process_tags(content, &tags(&["windows"])).unwrap();
        assert!(!result2.content.contains("unix-only config"));
    }

    #[test]
    fn preserves_indentation() {
        let content = "\
# {linux-
    indented content
        more indented
# -linux}
";
        let result = process_tags(content, &tags(&["linux"])).unwrap();
        assert!(result.content.contains("    indented content"));
        assert!(result.content.contains("        more indented"));
    }
}
