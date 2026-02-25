pub mod matcher;
pub mod parser;

use std::collections::HashSet;

/// A boolean expression over build tags.
///
/// Supports `&` (and), `|` (or), `!` (not), and parenthesised grouping.
/// Evaluated against a set of active tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagExpr {
    /// A single tag literal, e.g. `"linux"`
    Tag(String),
    /// Logical NOT, e.g. `!windows`
    Not(Box<TagExpr>),
    /// Logical AND, e.g. `linux & laptop`
    And(Box<TagExpr>, Box<TagExpr>),
    /// Logical OR, e.g. `macos | default`
    Or(Box<TagExpr>, Box<TagExpr>),
}

impl TagExpr {
    /// Evaluate this expression against a set of active tags.
    pub fn evaluate(&self, active_tags: &HashSet<String>) -> bool {
        match self {
            TagExpr::Tag(name) => active_tags.contains(name),
            TagExpr::Not(inner) => !inner.evaluate(active_tags),
            TagExpr::And(lhs, rhs) => lhs.evaluate(active_tags) && rhs.evaluate(active_tags),
            TagExpr::Or(lhs, rhs) => lhs.evaluate(active_tags) || rhs.evaluate(active_tags),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn single_tag_present() {
        let expr = TagExpr::Tag("linux".into());
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
    }

    #[test]
    fn single_tag_absent() {
        let expr = TagExpr::Tag("windows".into());
        assert!(!expr.evaluate(&tags(&["linux", "laptop"])));
    }

    #[test]
    fn not_expr() {
        let expr = TagExpr::Not(Box::new(TagExpr::Tag("windows".into())));
        assert!(expr.evaluate(&tags(&["linux"])));
        assert!(!expr.evaluate(&tags(&["windows"])));
    }

    #[test]
    fn and_expr() {
        let expr = TagExpr::And(
            Box::new(TagExpr::Tag("linux".into())),
            Box::new(TagExpr::Tag("laptop".into())),
        );
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
        assert!(!expr.evaluate(&tags(&["linux", "desktop"])));
    }

    #[test]
    fn or_expr() {
        let expr = TagExpr::Or(
            Box::new(TagExpr::Tag("macos".into())),
            Box::new(TagExpr::Tag("default".into())),
        );
        assert!(expr.evaluate(&tags(&["macos"])));
        assert!(expr.evaluate(&tags(&["default"])));
        assert!(!expr.evaluate(&tags(&["linux"])));
    }

    #[test]
    fn complex_expr() {
        // linux & (laptop | desktop)
        let expr = TagExpr::And(
            Box::new(TagExpr::Tag("linux".into())),
            Box::new(TagExpr::Or(
                Box::new(TagExpr::Tag("laptop".into())),
                Box::new(TagExpr::Tag("desktop".into())),
            )),
        );
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
        assert!(expr.evaluate(&tags(&["linux", "desktop"])));
        assert!(!expr.evaluate(&tags(&["linux", "server"])));
        assert!(!expr.evaluate(&tags(&["macos", "laptop"])));
    }

    #[test]
    fn negation_in_and() {
        // linux & !server
        let expr = TagExpr::And(
            Box::new(TagExpr::Tag("linux".into())),
            Box::new(TagExpr::Not(Box::new(TagExpr::Tag("server".into())))),
        );
        assert!(expr.evaluate(&tags(&["linux", "laptop"])));
        assert!(!expr.evaluate(&tags(&["linux", "server"])));
    }
}
