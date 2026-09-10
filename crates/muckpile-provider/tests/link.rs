//! Deciding which of the provider's own relationship types matches a
//! requested phrase, and firing it in the right direction — no vocabulary
//! of muckpile's own (decision 8, extended from status to this).

use muckpile_provider::fake::FakeProvider;
use muckpile_provider::link::{link, Outcome};
use muckpile_provider::provider::Provider;

#[test]
fn the_outward_phrase_links_a_toward_b_as_the_outward_issue() {
    let p = FakeProvider::new();
    p.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);

    let outcome = link(&p, "ACC-229", "blocks", "ACC-338").unwrap();

    match outcome {
        Outcome::Applied { type_name } => assert_eq!(type_name, "Blocks"),
        Outcome::NoSuchPhrase { .. } => panic!("expected the link to apply"),
    }
    assert_eq!(p.links_created(), vec![("Blocks".to_string(), "ACC-229".to_string(), "ACC-338".to_string())]);
}

/// `depends` was never muckpile's own vocabulary to keep: the same
/// `Blocks` type, asked from its inward phrase, is exactly "A depends on B".
#[test]
fn the_inward_phrase_reverses_which_key_plays_outward() {
    let p = FakeProvider::new();
    p.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);

    let outcome = link(&p, "ACC-338", "is blocked by", "ACC-229").unwrap();

    match outcome {
        Outcome::Applied { type_name } => assert_eq!(type_name, "Blocks"),
        Outcome::NoSuchPhrase { .. } => panic!("expected the link to apply"),
    }
    // "ACC-338 is blocked by ACC-229" is the same edge as "ACC-229 blocks
    // ACC-338": the outward issue is ACC-229, regardless of argument order.
    assert_eq!(p.links_created(), vec![("Blocks".to_string(), "ACC-229".to_string(), "ACC-338".to_string())]);
}

#[test]
fn a_phrase_no_type_offers_lists_what_the_provider_does() {
    let p = FakeProvider::new();
    p.seed_link_types(&[("Blocks", "blocks", "is blocked by"), ("Relates", "relates to", "relates to")]);

    let outcome = link(&p, "ACC-1", "depends", "ACC-2").unwrap();

    match outcome {
        Outcome::NoSuchPhrase { available } => {
            assert!(available.contains(&"blocks".to_string()));
            assert!(available.contains(&"is blocked by".to_string()));
            assert!(available.contains(&"relates to".to_string()));
        }
        Outcome::Applied { .. } => panic!("\"depends\" isn't a phrase either seeded type offers"),
    }
    assert!(p.links_created().is_empty());
}

#[test]
fn a_type_whose_two_phrases_are_identical_lists_the_phrase_once() {
    let p = FakeProvider::new();
    p.seed_link_types(&[("Relates", "relates to", "relates to")]);

    let outcome = link(&p, "ACC-1", "nope", "ACC-2").unwrap();

    match outcome {
        Outcome::NoSuchPhrase { available } => assert_eq!(available, vec!["relates to".to_string()]),
        Outcome::Applied { .. } => panic!("\"nope\" isn't offered"),
    }
}

/// The header shows a phrase with `_` for each space, so a phrase copied
/// from there is the same phrase.
#[test]
fn a_phrase_with_underscores_for_spaces_is_the_same_phrase() {
    let p = FakeProvider::new();
    p.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);

    let outcome = link(&p, "ACC-338", "is_blocked_by", "ACC-229").unwrap();

    assert!(matches!(outcome, Outcome::Applied { .. }));
    assert_eq!(p.links_created(), vec![("Blocks".to_string(), "ACC-229".to_string(), "ACC-338".to_string())]);
}

/// The fake's `delete_link` undoes exactly what its `create_link` did: the
/// record, and the link on both items, each from its own side — and only in
/// the direction it was made.
#[test]
fn the_fake_deletes_a_link_from_the_record_and_from_both_items() {
    let p = FakeProvider::new();
    p.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    p.seed_item("ACC-229", "Tarea", "a", "Tareas por hacer", None, None);
    p.seed_item("ACC-338", "Tarea", "b", "Tareas por hacer", None, None);
    p.create_link("Blocks", "ACC-229", "ACC-338").unwrap();

    assert!(!p.delete_link("Blocks", "ACC-338", "ACC-229").unwrap());
    assert_eq!(p.links_created().len(), 1);

    assert!(p.delete_link("Blocks", "ACC-229", "ACC-338").unwrap());
    assert!(p.links_created().is_empty());
    assert!(p.item("ACC-229").unwrap().links.is_empty());
    assert!(p.item("ACC-338").unwrap().links.is_empty());
    assert!(!p.delete_link("Blocks", "ACC-229", "ACC-338").unwrap());
}
