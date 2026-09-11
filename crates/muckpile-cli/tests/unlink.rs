//! `unlink` removes what `link` creates — the deciding-and-removing logic
//! lives in `muckpile-provider::link`; this slice is only the id validation
//! the other CLI commands already do.

use muckpile_cli::{link, unlink};
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::link::UnlinkOutcome;

/// The person approved it, or the project writes on its own.
fn approved() -> anyhow::Result<()> {
    Ok(())
}

#[test]
fn unlinks_two_valid_ids_by_the_provider_s_phrase() {
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    link("ACC-229", "blocks", "ACC-338", &provider, approved).unwrap();

    let outcome = unlink("ACC-338", "is_blocked_by", "ACC-229", &provider, approved).unwrap();

    match outcome {
        UnlinkOutcome::Removed { type_name } => assert_eq!(type_name, "Blocks"),
        _ => panic!("expected the link to be removed"),
    }
    assert!(provider.links_created().is_empty());
}

#[test]
fn refuses_a_first_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    assert!(unlink("../escape", "blocks", "ACC-338", &provider, approved).is_err());
}

#[test]
fn refuses_a_second_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    assert!(unlink("ACC-229", "blocks", "../escape", &provider, approved).is_err());
}
