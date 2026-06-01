//! Simple `${{ var }}` template substitution engine.
//!
//! Uses `${{ var }}` delimiters (GitHub Actions style) to avoid collisions with
//! programming languages that use `{{ }}` (e.g. Lua nested tables, Jinja, Nunjucks).
//!
//! - Undefined variables are hard errors (no silent empty strings).
//! - Whitespace inside braces is trimmed: `${{ var }}` and `${{var}}` both work.
//! - Literal `${{` can be escaped as `\${{`.

use std::collections::HashMap;

use crate::error::{Result, TowboatError};

/// Substitute `${{ var }}` placeholders in content with values from `variables`.
///
/// Returns an error if any referenced variable is not defined.
pub fn render(content: &str, variables: &HashMap<String, String>) -> Result<String> {
    // Accumulate raw bytes. The template delimiters (`$`, `{`, `}`, `\`) are all
    // ASCII, and UTF-8 continuation bytes are always >= 0x80, so scanning by byte
    // never misidentifies a delimiter inside a multi-byte character. Passthrough
    // bytes must be copied verbatim — casting `byte as char` would reinterpret each
    // byte as a Latin-1 codepoint and re-encode it, corrupting multi-byte UTF-8.
    let mut output: Vec<u8> = Vec::with_capacity(content.len());
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'\\'
            && i + 3 < len
            && bytes[i + 1] == b'$'
            && bytes[i + 2] == b'{'
            && bytes[i + 3] == b'{'
        {
            // Escaped ${{ — emit literal ${{
            output.extend_from_slice(b"${{");
            i += 4;
        } else if bytes[i] == b'$'
            && i + 2 < len
            && bytes[i + 1] == b'{'
            && bytes[i + 2] == b'{'
        {
            // Opening ${{ — find closing }}
            i += 3;
            let start = i;
            let mut found_close = false;

            while i < len {
                if bytes[i] == b'}' && i + 1 < len && bytes[i + 1] == b'}' {
                    found_close = true;
                    let var_name = &content[start..i];
                    let var_name = var_name.trim();

                    if var_name.is_empty() {
                        output.extend_from_slice(b"${{}}");
                    } else {
                        match variables.get(var_name) {
                            Some(value) => output.extend_from_slice(value.as_bytes()),
                            None => {
                                return Err(TowboatError::UndefinedVariable {
                                    name: var_name.to_string(),
                                });
                            }
                        }
                    }
                    i += 2;
                    break;
                }
                i += 1;
            }

            if !found_close {
                // Unterminated ${{ — pass through literally
                output.extend_from_slice(b"${{");
                output.extend_from_slice(&bytes[start..]);
            }
        } else {
            output.push(bytes[i]);
            i += 1;
        }
    }

    // Safe: input is valid UTF-8 and we only ever append whole bytes from it or
    // valid UTF-8 from variable values and ASCII literals.
    Ok(String::from_utf8(output).expect("template render produced invalid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn basic_substitution() {
        let content = "Hello ${{ name }}!";
        let result = render(content, &vars(&[("name", "World")])).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn no_spaces_in_braces() {
        let content = "Hello ${{name}}!";
        let result = render(content, &vars(&[("name", "World")])).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn multiple_variables() {
        let content = "${{ greeting }}, ${{ name }}! Your email is ${{ email }}.";
        let result = render(
            content,
            &vars(&[
                ("greeting", "Hi"),
                ("name", "Alice"),
                ("email", "alice@example.com"),
            ]),
        )
        .unwrap();
        assert_eq!(result, "Hi, Alice! Your email is alice@example.com.");
    }

    #[test]
    fn undefined_variable_is_error() {
        let content = "Hello ${{ missing }}!";
        let result = render(content, &vars(&[]));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn no_templates_passthrough() {
        let content = "just a plain file\nwith no templates";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn escaped_braces() {
        let content = r"This is \${{ not a template }}";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, "This is ${{ not a template }}");
    }

    #[test]
    fn single_brace_passthrough() {
        let content = "a { b } c";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, "a { b } c");
    }

    #[test]
    fn double_brace_passthrough() {
        // Lua nested tables, Jinja, etc. should pass through untouched
        let content = "local t = {{ hostname }}";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, "local t = {{ hostname }}");
    }

    #[test]
    fn variable_in_multiline() {
        let content = "line1\nhost = ${{ hostname }}\nline3";
        let result = render(content, &vars(&[("hostname", "mybox")])).unwrap();
        assert_eq!(result, "line1\nhost = mybox\nline3");
    }

    #[test]
    fn variable_with_special_chars_in_value() {
        let content = "path = ${{ path }}";
        let result = render(content, &vars(&[("path", "/usr/local/bin")])).unwrap();
        assert_eq!(result, "path = /usr/local/bin");
    }

    #[test]
    fn empty_variable_name_passthrough() {
        let content = "test ${{}} end";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, "test ${{}} end");
    }

    #[test]
    fn dollar_without_braces_passthrough() {
        let content = "price is $50 and ${HOME}";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, "price is $50 and ${HOME}");
    }

    #[test]
    fn multibyte_utf8_passthrough_unchanged() {
        // Nerd-font glyphs and other multi-byte UTF-8 must survive verbatim,
        // not be reinterpreted byte-by-byte as Latin-1 (mojibake).
        let content = "return \u{F0109} .. café — naïve 日本語 🚀";
        let result = render(content, &vars(&[])).unwrap();
        assert_eq!(result, content);
        assert_eq!(result.as_bytes(), content.as_bytes());
    }

    #[test]
    fn multibyte_utf8_around_substitution() {
        let content = "icon \u{F0109} = ${{ name }} café 🚀";
        let result = render(content, &vars(&[("name", "timer")])).unwrap();
        assert_eq!(result, "icon \u{F0109} = timer café 🚀");
    }

    #[test]
    fn multibyte_variable_value() {
        let content = "glyph: ${{ icon }}";
        let result = render(content, &vars(&[("icon", "\u{F0109} 日本")])).unwrap();
        assert_eq!(result, "glyph: \u{F0109} 日本");
    }
}
