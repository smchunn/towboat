//! Property-based tests for towboat's core pure logic.

use std::collections::{HashMap, HashSet};

use proptest::prelude::*;

use towboat::tags::matcher;
use towboat::tags::parser;
use towboat::template::engine;

// --- Tag expression strategies ---

/// Generate a valid tag name (alphanumeric + hyphens + underscores, 1-20 chars).
fn tag_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,9}".prop_map(|s| s.to_string())
}

/// Generate a set of active tags (1-5 tags).
fn active_tags_strategy() -> impl Strategy<Value = HashSet<String>> {
    prop::collection::hash_set(tag_name_strategy(), 1..=5)
}

/// Generate a simple tag expression string (single tag, AND, OR, NOT).
fn simple_expr_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        tag_name_strategy(),
        (tag_name_strategy(), tag_name_strategy()).prop_map(|(a, b)| format!("{a} & {b}")),
        (tag_name_strategy(), tag_name_strategy()).prop_map(|(a, b)| format!("{a} | {b}")),
        tag_name_strategy().prop_map(|t| format!("!{t}")),
        (tag_name_strategy(), tag_name_strategy()).prop_map(|(a, b)| format!("({a} | {b})")),
    ]
}

// --- Tag matcher properties ---

proptest! {
    #[test]
    fn parsed_tag_expr_is_deterministic(
        expr_str in simple_expr_strategy(),
        tags in active_tags_strategy(),
    ) {
        // Parsing the same expression twice should yield the same evaluation
        let expr1 = matcher::parse(&expr_str).unwrap();
        let expr2 = matcher::parse(&expr_str).unwrap();
        prop_assert_eq!(expr1.evaluate(&tags), expr2.evaluate(&tags));
    }

    #[test]
    fn single_tag_evaluates_to_membership(tag in tag_name_strategy(), tags in active_tags_strategy()) {
        let expr = matcher::parse(&tag).unwrap();
        prop_assert_eq!(expr.evaluate(&tags), tags.contains(&tag));
    }

    #[test]
    fn not_tag_is_negation(tag in tag_name_strategy(), tags in active_tags_strategy()) {
        let expr = matcher::parse(&format!("!{tag}")).unwrap();
        prop_assert_eq!(expr.evaluate(&tags), !tags.contains(&tag));
    }

    #[test]
    fn and_requires_both(a in tag_name_strategy(), b in tag_name_strategy(), tags in active_tags_strategy()) {
        let expr = matcher::parse(&format!("{a} & {b}")).unwrap();
        let expected = tags.contains(&a) && tags.contains(&b);
        prop_assert_eq!(expr.evaluate(&tags), expected);
    }

    #[test]
    fn or_requires_either(a in tag_name_strategy(), b in tag_name_strategy(), tags in active_tags_strategy()) {
        let expr = matcher::parse(&format!("{a} | {b}")).unwrap();
        let expected = tags.contains(&a) || tags.contains(&b);
        prop_assert_eq!(expr.evaluate(&tags), expected);
    }

    #[test]
    fn double_negation_is_identity(tag in tag_name_strategy(), tags in active_tags_strategy()) {
        let expr = matcher::parse(&format!("!!{tag}")).unwrap();
        prop_assert_eq!(expr.evaluate(&tags), tags.contains(&tag));
    }

    #[test]
    fn or_is_commutative(a in tag_name_strategy(), b in tag_name_strategy(), tags in active_tags_strategy()) {
        let ab = matcher::parse(&format!("{a} | {b}")).unwrap();
        let ba = matcher::parse(&format!("{b} | {a}")).unwrap();
        prop_assert_eq!(ab.evaluate(&tags), ba.evaluate(&tags));
    }

    #[test]
    fn and_is_commutative(a in tag_name_strategy(), b in tag_name_strategy(), tags in active_tags_strategy()) {
        let ab = matcher::parse(&format!("{a} & {b}")).unwrap();
        let ba = matcher::parse(&format!("{b} & {a}")).unwrap();
        prop_assert_eq!(ab.evaluate(&tags), ba.evaluate(&tags));
    }
}

// --- Tag parser properties ---

proptest! {
    #[test]
    fn plain_content_passes_through(content in "[a-zA-Z0-9 =_./\n]{1,200}") {
        // Content without any tag markers should pass through unchanged
        let tags: HashSet<String> = ["default"].iter().map(|s| s.to_string()).collect();
        let result = parser::process_tags(&content, &tags).unwrap();
        prop_assert!(!result.had_tags);
        // Account for trailing newline normalization
        let expected = if content.ends_with('\n') {
            content.clone()
        } else {
            content.clone()
        };
        prop_assert_eq!(result.content, expected);
    }

    #[test]
    fn matching_section_content_is_included(
        tag in tag_name_strategy(),
        body in "[a-zA-Z0-9 =_.]{1,50}",
    ) {
        // Prefix body with unique marker to avoid false substring matches
        // against the surrounding "XXX"/"YYY" text
        let body_line = format!("BODY_{body}");
        let content = format!("XXX\n# {{{tag}-\n{body_line}\n# -{tag}}}\nYYY\n");
        let tags: HashSet<String> = [tag.as_str()].iter().map(|s| s.to_string()).collect();
        let result = parser::process_tags(&content, &tags).unwrap();
        prop_assert!(result.had_tags);
        prop_assert!(result.content.contains(&body_line));
        prop_assert!(result.content.contains("XXX"));
        prop_assert!(result.content.contains("YYY"));
    }

    #[test]
    fn non_matching_section_content_is_excluded(
        tag in tag_name_strategy(),
        body in "[a-zA-Z0-9 =_.]{1,50}",
    ) {
        let body_line = format!("BODY_{body}");
        let content = format!("XXX\n# {{{tag}-\n{body_line}\n# -{tag}}}\nYYY\n");
        let tags: HashSet<String> = ["__nonexistent__"].iter().map(|s| s.to_string()).collect();
        let result = parser::process_tags(&content, &tags).unwrap();
        prop_assert!(result.had_tags);
        prop_assert!(!result.content.contains(&body_line));
        prop_assert!(result.content.contains("XXX"));
        prop_assert!(result.content.contains("YYY"));
    }
}

// --- Template engine properties ---

proptest! {
    #[test]
    fn template_substitution_replaces_variable(
        var_name in "[a-z_]{1,10}",
        var_value in "[a-zA-Z0-9_. -]{0,50}",
    ) {
        let content = format!("prefix ${{{{ {var_name} }}}} suffix");
        let mut vars = HashMap::new();
        vars.insert(var_name.clone(), var_value.clone());
        let result = engine::render(&content, &vars).unwrap();
        prop_assert_eq!(result, format!("prefix {var_value} suffix"));
    }

    #[test]
    fn no_templates_means_passthrough(content in "[a-zA-Z0-9 =_./\n]{1,200}") {
        // Content without ${{ should pass through unchanged
        prop_assume!(!content.contains("${{"));
        let vars: HashMap<String, String> = HashMap::new();
        let result = engine::render(&content, &vars).unwrap();
        prop_assert_eq!(result, content);
    }

    #[test]
    fn escaped_braces_produce_literal(prefix in "[a-z]{1,10}", suffix in "[a-z]{1,10}") {
        let content = format!("{prefix} \\${{{{ not_a_var }}}} {suffix}");
        let vars: HashMap<String, String> = HashMap::new();
        let result = engine::render(&content, &vars).unwrap();
        prop_assert!(result.contains("${{"));
    }

    #[test]
    fn undefined_variable_is_error(var_name in "[a-z_]{1,10}") {
        let content = format!("value = ${{{{ {var_name} }}}}");
        let vars: HashMap<String, String> = HashMap::new();
        let result = engine::render(&content, &vars);
        prop_assert!(result.is_err());
    }
}
