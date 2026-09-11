//! `link` replaces `depends`/`blocks` as muckpile's own vocabulary — the
//! deciding-and-firing logic already lives in `muckpile-provider::link`;
//! this slice is only the id validation the other CLI commands already do.

use muckpile_cli::link;
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::link::Outcome;

/// The person approved it, or the project writes on its own.
fn approved() -> anyhow::Result<()> {
    Ok(())
}

#[test]
fn links_two_valid_ids_by_the_provider_s_phrase() {
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);

    let outcome = link("ACC-229", "blocks", "ACC-338", &provider, approved).unwrap();

    match outcome {
        Outcome::Applied { type_name } => assert_eq!(type_name, "Blocks"),
        Outcome::NoSuchPhrase { .. } => panic!("expected the link to apply"),
    }
}

#[test]
fn refuses_a_first_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    assert!(link("../escape", "blocks", "ACC-338", &provider, approved).is_err());
}

#[test]
fn refuses_a_second_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    assert!(link("ACC-229", "blocks", "../escape", &provider, approved).is_err());
}
