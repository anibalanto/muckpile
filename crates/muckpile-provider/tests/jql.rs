//! `search_text` reduces a title to something JQL's `summary ~` full-text
//! search can parse, without pretending to be an exact match — the exact
//! match happens by comparing `summary` field-for-field after the search
//! comes back.

use muckpile_provider::jql::search_text;

#[test]
fn plain_text_survives_untouched() {
    assert_eq!(search_text("Vistas de trabajo"), "Vistas de trabajo");
}

#[test]
fn a_single_hyphen_is_kept() {
    assert_eq!(search_text("bilinker-002-file-partition"), "bilinker-002-file-partition");
}

#[test]
fn a_run_of_two_or_more_hyphens_is_dropped_as_a_run() {
    assert_eq!(search_text("graph --format json"), "graph format json");
    assert_eq!(search_text("a --b"), "a b");
    assert_eq!(search_text("a ---b"), "a b");
}

#[test]
fn characters_that_break_the_jql_parser_become_a_space() {
    for c in ['[', ']', '(', ')', '{', '}', '^', '"', '*'] {
        assert!(!search_text(&format!("a{c}b")).contains(c), "{c:?} survived");
    }
}

#[test]
fn a_dropped_character_next_to_a_real_space_leaves_the_usual_double_space() {
    assert_eq!(search_text("[prueba] algo"), "prueba  algo", "the double space is the ordinary one");
}

#[test]
fn a_backslash_becomes_a_space() {
    assert_eq!(search_text(r"a\b"), "a b");
}

#[test]
fn accents_are_kept() {
    assert_eq!(search_text("Índice"), "Índice");
}

#[test]
fn leading_and_trailing_whitespace_from_a_dropped_run_is_trimmed() {
    assert_eq!(search_text("--leading trailing--"), "leading trailing");
}

#[test]
fn only_punctuation_reduces_to_empty() {
    assert_eq!(search_text("---"), "");
    assert_eq!(search_text("[]{}()"), "");
}
