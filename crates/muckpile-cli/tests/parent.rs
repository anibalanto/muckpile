//! `parent`: the only way the parent of an already-synced item changes —
//! editing `parent:` in the header isn't sent by `push`.

use muckpile_cli::parent;
use muckpile_provider::fake::FakeProvider;

/// The person approved it, or the project writes on its own.
fn approved() -> anyhow::Result<()> {
    Ok(())
}

#[test]
fn writes_the_new_parent_to_the_provider() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", Some("ACC-100"), None);

    parent("ACC-355", "ACC-339", &provider, approved).unwrap();

    assert_eq!(provider.parent_of("ACC-355").as_deref(), Some("ACC-339"));
}

#[test]
fn gives_a_parent_to_an_item_that_had_none() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", None, None);

    parent("ACC-355", "ACC-339", &provider, approved).unwrap();

    assert_eq!(provider.parent_of("ACC-355").as_deref(), Some("ACC-339"));
}

#[test]
fn refuses_an_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", None, None);

    assert!(parent("../escape", "ACC-339", &provider, approved).is_err());
    assert!(parent("ACC-355", "../escape", &provider, approved).is_err());
    assert_eq!(provider.parent_of("ACC-355"), None);
}

#[test]
fn refuses_an_item_as_its_own_parent() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", Some("ACC-100"), None);

    let err = parent("ACC-355", "ACC-355", &provider, approved).unwrap_err();

    assert!(err.to_string().contains("ACC-355"), "{err}");
    assert_eq!(provider.parent_of("ACC-355").as_deref(), Some("ACC-100"));
}

#[test]
fn an_item_the_provider_doesn_t_have_fails() {
    let provider = FakeProvider::new();
    assert!(parent("ACC-355", "ACC-339", &provider, approved).is_err());
}
