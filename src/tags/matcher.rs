//! Recursive-descent parser for tag expressions.
//!
//! Grammar (precedence low→high):
//!
//! ```text
//! expr   = or_expr
//! or     = and ("|" and)*
//! and    = unary ("&" unary)*
//! unary  = "!" unary | atom
//! atom   = TAG | "(" expr ")"
//! TAG    = [a-zA-Z0-9_-]+
//! ```

use crate::error::TowboatError;
use crate::tags::TagExpr;

/// Parse a tag expression string into a `TagExpr`.
///
/// Examples: `"linux"`, `"linux & laptop"`, `"macos | default"`,
/// `"!windows"`, `"linux & (laptop | desktop)"`
pub fn parse(input: &str) -> crate::error::Result<TagExpr> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let expr = parse_or(&tokens, &mut pos)?;

    if pos < tokens.len() {
        return Err(TowboatError::InvalidTagExpr(format!(
            "unexpected token {:?} at position {}",
            tokens[pos], pos
        )));
    }

    Ok(expr)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Tag(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> crate::error::Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                chars.next();
            }
            '&' => {
                chars.next();
                tokens.push(Token::And);
            }
            '|' => {
                chars.next();
                tokens.push(Token::Or);
            }
            '!' => {
                chars.next();
                tokens.push(Token::Not);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            c if c.is_alphanumeric() || c == '_' || c == '-' => {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '-' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Tag(name));
            }
            other => {
                return Err(TowboatError::InvalidTagExpr(format!(
                    "unexpected character '{other}'"
                )));
            }
        }
    }

    Ok(tokens)
}

fn parse_or(tokens: &[Token], pos: &mut usize) -> crate::error::Result<TagExpr> {
    let mut left = parse_and(tokens, pos)?;

    while *pos < tokens.len() && tokens[*pos] == Token::Or {
        *pos += 1;
        let right = parse_and(tokens, pos)?;
        left = TagExpr::Or(Box::new(left), Box::new(right));
    }

    Ok(left)
}

fn parse_and(tokens: &[Token], pos: &mut usize) -> crate::error::Result<TagExpr> {
    let mut left = parse_unary(tokens, pos)?;

    while *pos < tokens.len() && tokens[*pos] == Token::And {
        *pos += 1;
        let right = parse_unary(tokens, pos)?;
        left = TagExpr::And(Box::new(left), Box::new(right));
    }

    Ok(left)
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> crate::error::Result<TagExpr> {
    if *pos < tokens.len() && tokens[*pos] == Token::Not {
        *pos += 1;
        let inner = parse_unary(tokens, pos)?;
        return Ok(TagExpr::Not(Box::new(inner)));
    }

    parse_atom(tokens, pos)
}

fn parse_atom(tokens: &[Token], pos: &mut usize) -> crate::error::Result<TagExpr> {
    if *pos >= tokens.len() {
        return Err(TowboatError::InvalidTagExpr(
            "unexpected end of expression".into(),
        ));
    }

    match &tokens[*pos] {
        Token::Tag(name) => {
            let expr = TagExpr::Tag(name.clone());
            *pos += 1;
            Ok(expr)
        }
        Token::LParen => {
            *pos += 1;
            let expr = parse_or(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err(TowboatError::InvalidTagExpr(
                    "missing closing parenthesis".into(),
                ));
            }
            *pos += 1;
            Ok(expr)
        }
        other => Err(TowboatError::InvalidTagExpr(format!(
            "unexpected token {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn tags(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_single_tag() {
        let expr = parse("linux").unwrap();
        assert_eq!(expr, TagExpr::Tag("linux".into()));
    }

    #[test]
    fn parse_and() {
        let expr = parse("linux & laptop").unwrap();
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
        assert!(!expr.evaluate(&tags(&["linux"])));
    }

    #[test]
    fn parse_or() {
        let expr = parse("macos | default").unwrap();
        assert!(expr.evaluate(&tags(&["macos"])));
        assert!(expr.evaluate(&tags(&["default"])));
        assert!(!expr.evaluate(&tags(&["linux"])));
    }

    #[test]
    fn parse_not() {
        let expr = parse("!windows").unwrap();
        assert!(expr.evaluate(&tags(&["linux"])));
        assert!(!expr.evaluate(&tags(&["windows"])));
    }

    #[test]
    fn parse_complex() {
        let expr = parse("linux & (laptop | desktop)").unwrap();
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
        assert!(expr.evaluate(&tags(&["linux", "desktop"])));
        assert!(!expr.evaluate(&tags(&["linux", "server"])));
        assert!(!expr.evaluate(&tags(&["macos", "laptop"])));
    }

    #[test]
    fn parse_not_with_and() {
        let expr = parse("linux & !server").unwrap();
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
        assert!(!expr.evaluate(&tags(&["linux", "server"])));
    }

    #[test]
    fn precedence_and_before_or() {
        // a | b & c  should parse as  a | (b & c)
        let expr = parse("a | b & c").unwrap();
        assert!(expr.evaluate(&tags(&["a"])));
        assert!(expr.evaluate(&tags(&["b", "c"])));
        assert!(!expr.evaluate(&tags(&["b"])));
    }

    #[test]
    fn double_not() {
        let expr = parse("!!linux").unwrap();
        assert!(expr.evaluate(&tags(&["linux"])));
        assert!(!expr.evaluate(&tags(&["macos"])));
    }

    #[test]
    fn tag_with_hyphen_and_underscore() {
        let expr = parse("my-tag & another_tag").unwrap();
        assert!(expr.evaluate(&tags(&["my-tag", "another_tag"])));
    }

    #[test]
    fn empty_input_is_error() {
        assert!(parse("").is_err());
    }

    #[test]
    fn unmatched_paren_is_error() {
        assert!(parse("(linux").is_err());
    }

    #[test]
    fn trailing_operator_is_error() {
        assert!(parse("linux &").is_err());
    }

    #[test]
    fn nested_parens() {
        let expr = parse("((linux))").unwrap();
        assert_eq!(expr, TagExpr::Tag("linux".into()));
    }

    #[test]
    fn complex_nested() {
        // (a | b) & (c | d)
        let expr = parse("(a | b) & (c | d)").unwrap();
        assert!(expr.evaluate(&tags(&["a", "c"])));
        assert!(expr.evaluate(&tags(&["b", "d"])));
        assert!(!expr.evaluate(&tags(&["a"])));
        assert!(!expr.evaluate(&tags(&["c"])));
    }
}
