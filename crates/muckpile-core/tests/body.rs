//! The body travels to the provider as ADF and what gets saved is what comes
//! back, not what was typed — so the round-trip has to converge, and
//! `canonical` has to agree with whatever `push` later decides is safe to
//! edit.

use muckpile_core::body::{canonical, line_diff, prune_marks, split_frontmatter};

const ITEM: &str = "---\ntitle: With structure\nstatus: Open\n---\n\n# Title\n\nWith *italic*, **bold** and `SNAKE_CASE` inside a code span.\n\n| Field | Owner |\n|---|---|\n| `status` | provider |\n\n> A blockquote.\n\n- a list\n- with two items\n";

#[test]
fn frontmatter_never_reaches_the_converter() {
    let (fm, body) = split_frontmatter(ITEM);
    assert!(fm.starts_with("---\n") && fm.ends_with("---\n"));
    assert!(fm.contains("title: With structure"));
    assert!(!body.contains("title:"));
}

#[test]
fn text_without_frontmatter_keeps_it_empty() {
    let (fm, body) = split_frontmatter("no frontmatter here");
    assert_eq!(fm, "");
    assert_eq!(body, "no frontmatter here");
}

#[test]
fn canonical_reproduces_the_frontmatter_byte_for_byte() {
    let (fm, _) = split_frontmatter(ITEM);
    let out = canonical(ITEM).unwrap();
    assert!(out.starts_with(fm), "the frontmatter has to come back untouched");
}

#[test]
fn canonical_is_a_fixed_point() {
    let once = canonical(ITEM).unwrap();
    let twice = canonical(&once).unwrap();
    assert_eq!(once, twice, "canonicalizing twice is the same as once");
}

#[test]
fn structure_survives_into_the_adf() {
    let adf = muckpile_core::body::body_to_adf(ITEM).unwrap();
    for node in ["heading", "table", "blockquote", "bulletList"] {
        assert!(adf.contains(node), "missing {node} in the ADF");
    }
}

#[test]
fn snake_case_inside_a_code_span_survives() {
    let out = canonical(ITEM).unwrap();
    assert!(out.contains("`SNAKE_CASE`"));
}

/// ADF cannot combine `code` with `strong` or `em` — Jira rejects the whole
/// document, not just that node — so the two marks never coexist on a text
/// node once the conversion is done.
#[test]
fn code_and_strong_together_lose_the_strong() {
    let adf = muckpile_core::body::body_to_adf("Load **`bilinker`** first.").unwrap();
    let v: serde_json::Value = serde_json::from_str(&adf).unwrap();
    let mut seen = false;
    fn walk(n: &serde_json::Value, seen: &mut bool) {
        if n.get("type").and_then(|t| t.as_str()) == Some("text") {
            let marks: Vec<&str> = n
                .get("marks")
                .and_then(|m| m.as_array())
                .map(|a| a.iter().filter_map(|m| m["type"].as_str()).collect())
                .unwrap_or_default();
            if marks.contains(&"code") {
                *seen = true;
                assert!(!marks.contains(&"strong"), "strong survived next to code: {marks:?}");
            }
        }
        for c in n.get("content").and_then(|c| c.as_array()).into_iter().flatten() {
            walk(c, seen);
        }
    }
    walk(&v, &mut seen);
    assert!(seen, "the case was never exercised: no code mark appeared");
}

#[test]
fn strong_on_its_own_survives() {
    let adf = muckpile_core::body::body_to_adf("This is **important**.").unwrap();
    assert!(adf.contains("strong"), "{adf}");
}

#[test]
fn code_on_its_own_survives() {
    let adf = muckpile_core::body::body_to_adf("Run `bilinker check`.").unwrap();
    assert!(adf.contains("code"), "{adf}");
}

#[test]
fn pruning_reaches_inside_a_list() {
    let adf = muckpile_core::body::body_to_adf("- **`bilinker`** — the prerequisite\n- something else").unwrap();
    let v: serde_json::Value = serde_json::from_str(&adf).unwrap();
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains("bulletList"), "the case was never exercised: {s}");
    assert!(!s.contains("\"strong\""), "a strong survived inside the list: {s}");
}

#[test]
fn line_diff_marks_unchanged_lines_with_no_prefix() {
    let diff = line_diff("a\nb\nc\n", "a\nb\nc\n");
    assert_eq!(diff, "  a\n  b\n  c");
}

#[test]
fn line_diff_marks_a_replaced_line_as_removed_then_added() {
    let diff = line_diff("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(diff, "  a\n- b\n+ B\n  c");
}

#[test]
fn line_diff_marks_a_dropped_line_as_removed_only() {
    let diff = line_diff("a\nb\nc\n", "a\nc\n");
    assert_eq!(diff, "  a\n- b\n  c");
}

#[test]
fn line_diff_marks_an_inserted_line_as_added_only() {
    let diff = line_diff("a\nc\n", "a\nb\nc\n");
    assert_eq!(diff, "  a\n+ b\n  c");
}

#[test]
fn prune_marks_drops_em_and_strong_next_to_code() {
    let mut node = serde_json::json!({
        "type": "text",
        "text": "x",
        "marks": [{"type": "code"}, {"type": "strong"}, {"type": "em"}]
    });
    prune_marks(&mut node);
    let marks: Vec<&str> =
        node["marks"].as_array().unwrap().iter().filter_map(|m| m["type"].as_str()).collect();
    assert_eq!(marks, vec!["code"]);
}
