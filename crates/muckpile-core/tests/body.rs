//! The body travels to the provider as ADF and what gets saved is what comes
//! back, not what was typed — so whether a body can be edited locally is
//! decided on the provider's ADF, by whether it survives the trip through
//! markdown and back.

use muckpile_core::body::{adf_diff, adf_to_body, body_to_adf, cards_to_file_links, cited_keys, file_links_to_cards, line_diff, prune_marks, split_frontmatter, JiraAdfMarkdownFilter, Loss};

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
fn structure_survives_into_the_adf() {
    let adf = muckpile_core::body::body_to_adf(ITEM).unwrap();
    for node in ["heading", "table", "blockquote", "bulletList"] {
        assert!(adf.contains(node), "missing {node} in the ADF");
    }
}

/// Wraps a list of ADF block nodes, written as JSON, in a document.
fn doc(blocks: &str) -> String {
    format!(r#"{{"version":1,"type":"doc","content":[{blocks}]}}"#)
}

const PARAGRAPH: &str = r#"{"type":"paragraph","content":[{"type":"text","text":"With "},{"type":"text","text":"SNAKE_CASE","marks":[{"type":"code"}]},{"type":"text","text":" inside a code span."}]}"#;

/// Jira keeps a numbered-rows flag on the table itself; markdown has no
/// syntax for it, and the converter says so.
const NUMBERED_TABLE: &str = r#"{"type":"table","attrs":{"isNumberColumnEnabled":true,"layout":"default"},"content":[
    {"type":"tableRow","content":[{"type":"tableHeader","attrs":{},"content":[{"type":"paragraph","content":[{"type":"text","text":"field"}]}]}]},
    {"type":"tableRow","content":[{"type":"tableCell","attrs":{},"content":[{"type":"paragraph","content":[{"type":"text","text":"status"}]}]}]}]}"#;

#[test]
fn a_plain_body_is_canonical() {
    let filtered = JiraAdfMarkdownFilter::filter(&doc(PARAGRAPH)).unwrap();
    assert_eq!(filtered.losses, vec![]);
    assert!(filtered.markdown.contains("`SNAKE_CASE`"), "{}", filtered.markdown);
}

/// The three things the converter normalizes on reading ADF are
/// equivalences, not losses: an empty `attrs` on every table cell, a
/// paragraph's `localId`, and the space a bold run keeps at its edge when a
/// code span cuts it.
#[test]
fn what_the_converter_normalizes_is_not_a_loss() {
    let adf = doc(r#"
        {"type":"paragraph","attrs":{"localId":"a1b2"},"content":[
          {"type":"text","text":"The first endpoint is ","marks":[{"type":"strong"}]},
          {"type":"text","text":"reach","marks":[{"type":"code"}]},
          {"type":"text","text":", and it fails.","marks":[{"type":"strong"}]}]},
        {"type":"table","content":[
          {"type":"tableRow","content":[{"type":"tableHeader","attrs":{},"content":[{"type":"paragraph","content":[{"type":"text","text":"field"}]}]}]},
          {"type":"tableRow","content":[{"type":"tableCell","attrs":{},"content":[{"type":"paragraph","content":[{"type":"text","text":"status"}]}]}]}]}"#);
    assert_eq!(JiraAdfMarkdownFilter::filter(&adf).unwrap().losses, vec![]);
}

/// The table's markdown is exactly the one an unnumbered table gives — so a
/// check that goes markdown to ADF and back to markdown lets it through, and
/// the numbering is gone after the next write.
#[test]
fn a_table_with_numbered_rows_is_not_canonical() {
    let filtered = JiraAdfMarkdownFilter::filter(&doc(NUMBERED_TABLE)).unwrap();
    assert!(
        filtered.losses.iter().any(|l| matches!(l, Loss::Lossy(w) if w.contains("numbered"))),
        "{:?}",
        filtered.losses
    );
}

/// Leading whitespace in a paragraph has nowhere to live in markdown: it
/// comes back trimmed, with no warning from the converter. What catches it is
/// comparing the document, not the warnings.
#[test]
fn a_document_that_comes_back_different_is_not_canonical_even_without_a_warning() {
    let adf = doc(r#"{"type":"paragraph","content":[{"type":"text","text":"  indented"}]}"#);
    assert_eq!(JiraAdfMarkdownFilter::filter(&adf).unwrap().losses, vec![Loss::Differs]);
}

#[test]
fn the_diff_against_the_real_adf_shows_what_writing_the_draft_would_lose() {
    let real = doc(NUMBERED_TABLE);
    let draft = JiraAdfMarkdownFilter::filter(&real).unwrap().markdown;
    let diff = adf_diff(&real, &body_to_adf(&draft).unwrap()).unwrap();
    assert!(diff.lines().any(|l| l.starts_with('-') && l.contains("isNumberColumnEnabled")), "{diff}");
}

/// The empty `attrs` Jira puts on every cell isn't a difference: the real
/// ADF is compared in the converter's canonical form.
#[test]
fn the_diff_against_the_real_adf_leaves_out_what_the_converter_normalizes() {
    let real = doc(r#"{"type":"table","content":[
        {"type":"tableRow","content":[{"type":"tableHeader","attrs":{},"content":[{"type":"paragraph","content":[{"type":"text","text":"field"}]}]}]},
        {"type":"tableRow","content":[{"type":"tableCell","attrs":{},"content":[{"type":"paragraph","content":[{"type":"text","text":"status"}]}]}]}]}"#);
    let draft = JiraAdfMarkdownFilter::filter(&real).unwrap().markdown;
    let diff = adf_diff(&real, &body_to_adf(&draft).unwrap()).unwrap();
    assert!(!diff.lines().any(|l| l.starts_with('-') || l.starts_with('+')), "{diff}");
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

/// Jira stores a bold sentence cut by an inline code span as two strong runs,
/// the space left inside the first — measured on real items. Written as
/// `**text **`, CommonMark doesn't close the bold and it comes back as
/// literal asterisks.
#[test]
fn a_bold_run_cut_by_code_reads_back_as_bold() {
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[
        {"type":"text","text":"The first endpoint is ","marks":[{"type":"strong"}]},
        {"type":"text","text":"reach","marks":[{"type":"code"}]},
        {"type":"text","text":", and it fails.","marks":[{"type":"strong"}]}]}]}"#;
    let md = muckpile_core::body::adf_to_body(adf).unwrap();
    assert_eq!(md.trim_end(), "**The first endpoint is** `reach`**, and it fails.**");
}

/// Markdown tables can't span cells; the table has to travel whole instead of
/// coming back with its merged cell split.
#[test]
fn a_table_with_merged_cells_survives_the_trip_through_markdown() {
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"table","content":[
        {"type":"tableRow","content":[
          {"type":"tableHeader","attrs":{"colspan":2},"content":[
            {"type":"paragraph","content":[{"type":"text","text":"mode"}]}]}]},
        {"type":"tableRow","content":[
          {"type":"tableCell","content":[{"type":"paragraph","content":[{"type":"text","text":"dev"}]}]},
          {"type":"tableCell","content":[{"type":"paragraph","content":[{"type":"text","text":"iap"}]}]}]}]}]}"#;
    let md = muckpile_core::body::adf_to_body(adf).unwrap();
    let back = muckpile_core::body::body_to_adf(&md).unwrap();
    assert!(back.contains("\"colspan\":2"), "the span was lost:\n{md}");
}

fn url_of(key: &str) -> String {
    format!("https://x.atlassian.net/browse/{key}")
}

/// Only this project's items: another project's card stays a card.
fn key_of(url: &str) -> Option<String> {
    url.strip_prefix("https://x.atlassian.net/browse/").filter(|k| k.starts_with("ACC-")).map(str::to_string)
}

fn file_of(key: &str) -> Option<String> {
    match key {
        "ACC-338" => Some("ACC-338.task.md".into()),
        "ACC-346" => Some("ACC-346.epic.md".into()),
        _ => None,
    }
}

/// In Jira a link to an item's file leads nowhere; a card shows its key,
/// title and status. The link's text doesn't travel — the card shows the
/// title.
#[test]
fn a_link_to_an_item_file_goes_up_as_a_card() {
    let adf = body_to_adf("See [ACC-338](ACC-338.task.md), [the story](../ACC-339.user-story.md) and [docs](https://example.com).\n").unwrap();
    let out = file_links_to_cards(&adf, url_of).unwrap();
    assert!(out.contains(r#""type":"inlineCard""#), "{out}");
    assert!(out.contains("https://x.atlassian.net/browse/ACC-338") && out.contains("https://x.atlassian.net/browse/ACC-339"), "{out}");
    assert!(!out.contains("the story"), "the text doesn't travel: {out}");
    assert!(out.contains("https://example.com"), "any other link stays: {out}");
}

const CITING: &str = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[
    {"type":"inlineCard","attrs":{"url":"https://x.atlassian.net/browse/ACC-338"}},
    {"type":"text","text":" and "},
    {"type":"text","text":"ACC-346 The ","marks":[{"type":"link","attrs":{"href":"https://x.atlassian.net/browse/ACC-346"}}]},
    {"type":"text","text":"title","marks":[{"type":"code"},{"type":"link","attrs":{"href":"https://x.atlassian.net/browse/ACC-346"}}]},
    {"type":"text","text":" and "},
    {"type":"inlineCard","attrs":{"url":"https://x.atlassian.net/browse/SGE-1"}}]}]}"#;

#[test]
fn the_keys_a_body_cites_are_this_project_s_cards_and_links() {
    let keys: Vec<String> = cited_keys(CITING, key_of).unwrap().into_iter().collect();
    assert_eq!(keys, vec!["ACC-338".to_string(), "ACC-346".to_string()]);
}

/// A card, or an ordinary link — worklist left those, split in several text
/// runs when the title has code in it — comes down as one link to the
/// item's file, with the key as its text.
#[test]
fn a_card_or_a_link_to_an_item_comes_down_as_a_link_to_its_file() {
    let out = cards_to_file_links(CITING, key_of, file_of).unwrap();
    let markdown = adf_to_body(&out).unwrap();
    assert!(markdown.contains("[ACC-338](ACC-338.task.md)"), "{markdown}");
    assert!(markdown.contains("[ACC-346](ACC-346.epic.md)"), "one link, not one per run: {markdown}");
    assert!(markdown.contains("SGE-1"), "another project's card stays: {markdown}");
}

#[test]
fn down_and_up_again_gives_back_the_cards() {
    let down = adf_to_body(&cards_to_file_links(CITING, key_of, file_of).unwrap()).unwrap();
    let up = file_links_to_cards(&body_to_adf(&down).unwrap(), url_of).unwrap();
    assert_eq!(up.matches(r#""type":"inlineCard""#).count(), 3, "{up}");
}
