//! `title`: the only way an item's title changes — editing `title:` in the
//! header isn't sent by `push`.

use muckpile_cli::title;
use muckpile_provider::fake::FakeProvider;

/// The person approved it, or the project writes on its own.
fn approved() -> anyhow::Result<()> {
    Ok(())
}

#[test]
fn writes_the_new_title_to_the_provider() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", None, None);

    title("ACC-355", "Vistas de trabajo, con su ítem", &provider, approved).unwrap();

    assert_eq!(provider.title_of("ACC-355").as_deref(), Some("Vistas de trabajo, con su ítem"));
}

#[test]
fn refuses_an_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    assert!(title("../escape", "x", &provider, approved).is_err());
}

#[test]
fn refuses_an_empty_title() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", None, None);

    assert!(title("ACC-355", "  ", &provider, approved).is_err());
    assert_eq!(provider.title_of("ACC-355").as_deref(), Some("Vistas"));
}
