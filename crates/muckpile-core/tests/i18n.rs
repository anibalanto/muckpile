//! What the user reads comes from one message file per language, `en` and
//! `es-AR`: the code names a message by its key, the file holds its text,
//! with the message's data between braces.

use muckpile_core::i18n::{catalog, lang_for, set_lang, t, t_in, translate, Catalog, Lang};
use muckpile_core::msg;
use std::collections::BTreeSet;

const EN: &str = include_str!("../i18n/en.toml");
const ES_AR: &str = include_str!("../i18n/es-AR.toml");

#[test]
fn muckpile_lang_comes_before_the_locale() {
    assert_eq!(lang_for(Some("es-AR"), Some("en_US.UTF-8"), None, Some("en_US.UTF-8")), Lang::EsAr);
    assert_eq!(lang_for(Some("en"), Some("es_AR.UTF-8"), Some("es_AR.UTF-8"), Some("es_AR.UTF-8")), Lang::En);
}

#[test]
fn the_locale_is_lc_all_then_lc_messages_then_lang() {
    assert_eq!(lang_for(None, Some("es_AR.UTF-8"), Some("en_US.UTF-8"), Some("en_US.UTF-8")), Lang::EsAr);
    assert_eq!(lang_for(None, Some("en_US.UTF-8"), Some("es_AR.UTF-8"), Some("es_AR.UTF-8")), Lang::En);
    assert_eq!(lang_for(None, None, Some("es_AR.UTF-8"), Some("en_US.UTF-8")), Lang::EsAr);
    assert_eq!(lang_for(None, None, None, Some("es_ES.UTF-8")), Lang::EsAr);
    assert_eq!(lang_for(None, None, None, Some("en_US.UTF-8")), Lang::En);
}

#[test]
fn an_empty_variable_counts_as_unset() {
    assert_eq!(lang_for(Some(""), Some(""), Some(""), Some("es_AR.UTF-8")), Lang::EsAr);
    assert_eq!(lang_for(Some(""), None, Some(""), Some("en_US.UTF-8")), Lang::En);
}

#[test]
fn anything_not_starting_with_es_is_english() {
    for value in ["C", "POSIX", "pt_BR.UTF-8", "fr", "en-GB"] {
        assert_eq!(lang_for(Some(value), None, None, None), Lang::En, "{value}");
    }
    for value in ["es", "es-AR", "es_AR.UTF-8", "es_MX"] {
        assert_eq!(lang_for(Some(value), None, None, None), Lang::EsAr, "{value}");
    }
}

#[test]
fn nothing_set_at_all_is_english() {
    assert_eq!(lang_for(None, None, None, None), Lang::En);
}

#[test]
fn placeholders_are_filled_with_the_message_data() {
    assert_eq!(t_in(Lang::EsAr, "id.invalid", &[("id", &"ACC 1")]), "ACC 1: no es un id válido");
    assert_eq!(t_in(Lang::En, "id.invalid", &[("id", &"ACC 1")]), "ACC 1: not a valid id");
}

#[test]
fn a_filled_value_is_never_read_as_a_placeholder_itself() {
    let catalog = Catalog::parse(r#"greet = "{a} and {b}""#).unwrap();
    assert_eq!(translate(&[&catalog], "greet", &[("a", &"{b}"), ("b", &"two")]), "{b} and two");
}

#[test]
fn a_placeholder_with_no_datum_is_left_as_written() {
    let catalog = Catalog::parse(r#"greet = "hello {who}""#).unwrap();
    assert_eq!(translate(&[&catalog], "greet", &[]), "hello {who}");
}

#[test]
fn a_message_missing_from_the_first_catalog_comes_from_the_next() {
    let es_ar = Catalog::parse(r#"only.en.missing = "está""#).unwrap();
    let en = Catalog::parse("only.en.missing = \"is here\"\nonly.in.en = \"{n} in en\"").unwrap();
    assert_eq!(translate(&[&es_ar, &en], "only.en.missing", &[]), "está");
    assert_eq!(translate(&[&es_ar, &en], "only.in.en", &[("n", &3)]), "3 in en");
}

#[test]
fn an_unknown_key_comes_back_as_itself() {
    let en = Catalog::parse(r#"known = "yes""#).unwrap();
    assert_eq!(translate(&[&en], "no.such.key", &[("id", &"x")]), "no.such.key");
    assert_eq!(t_in(Lang::EsAr, "no.such.key", &[]), "no.such.key");
}

#[test]
fn dotted_keys_and_tables_are_the_same_key() {
    let dotted = Catalog::parse(r#"push.sent = "sent""#).unwrap();
    let table = Catalog::parse("[push]\nsent = \"sent\"").unwrap();
    assert_eq!(dotted.get("push.sent"), Some("sent"));
    assert_eq!(table.get("push.sent"), Some("sent"));
}

#[test]
fn a_catalog_holds_only_text() {
    assert!(Catalog::parse("n = 3").is_err());
}

/// The only test that reads or sets the process-wide language — tests run
/// in parallel threads: with nobody choosing, the library speaks es-AR;
/// after a choice, the one chosen. `msg!` names each datum the way
/// `format!` does, by name alone or `name = value`.
#[test]
fn the_library_speaks_es_ar_until_told_otherwise() {
    assert_eq!(t("id.invalid", &[("id", &"x")]), "x: no es un id válido");
    let id = "ACC 1";
    assert_eq!(msg!("id.invalid", id), "ACC 1: no es un id válido");
    assert_eq!(msg!("id.invalid", id = "x y",), "x y: no es un id válido");

    set_lang(Lang::En);
    assert_eq!(t("id.invalid", &[("id", &"x")]), "x: not a valid id");
    assert_eq!(msg!("id.invalid", id), "ACC 1: not a valid id");

    set_lang(Lang::EsAr);
    assert_eq!(t("id.invalid", &[("id", &"x")]), "x: no es un id válido");
}

#[test]
fn both_files_parse_and_are_what_the_binary_carries() {
    let en = Catalog::parse(EN).unwrap();
    let es_ar = Catalog::parse(ES_AR).unwrap();
    assert_eq!(keys(&en), keys(catalog(Lang::En)));
    assert_eq!(keys(&es_ar), keys(catalog(Lang::EsAr)));
}

/// What keeps the two files honest: the same keys on both sides, and each
/// message carrying the same data in either language.
#[test]
fn every_message_is_in_both_languages_with_the_same_data() {
    let en = Catalog::parse(EN).unwrap();
    let es_ar = Catalog::parse(ES_AR).unwrap();

    let only_en: Vec<_> = keys(&en).difference(&keys(&es_ar)).cloned().collect();
    let only_es_ar: Vec<_> = keys(&es_ar).difference(&keys(&en)).cloned().collect();
    assert!(only_en.is_empty(), "missing from es-AR.toml: {only_en:?}");
    assert!(only_es_ar.is_empty(), "missing from en.toml: {only_es_ar:?}");

    let mismatched: Vec<_> = keys(&en)
        .into_iter()
        .filter(|key| placeholders(en.get(key).unwrap()) != placeholders(es_ar.get(key).unwrap()))
        .collect();
    assert!(mismatched.is_empty(), "different {{placeholders}} in en and es-AR: {mismatched:?}");
}

fn keys(catalog: &Catalog) -> BTreeSet<String> {
    catalog.keys().map(str::to_string).collect()
}

/// Every `{name}` in a message's text.
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        rest = &rest[open + 1..];
        if let Some(close) = rest.find('}') {
            let name = &rest[..close];
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.insert(name.to_string());
                rest = &rest[close + 1..];
            }
        }
    }
    out
}
