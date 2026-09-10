//! `show` prints frontmatter, body, and the `<id>_data/` listing when it
//! exists — live from the provider, or from the local copy with `--local`.

use muckpile_cli::{show_live, show_local};
use muckpile_provider::fake::FakeProvider;
use std::path::Path;

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn live_reads_the_provider_and_converts_the_body() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hola"}]}]}"#;
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", Some("ACC-100"), Some(adf));

    let show = show_live(dir.path(), "ACC-355", &provider).unwrap();

    assert_eq!(show.title, "Vistas de trabajo");
    assert_eq!(show.status.as_deref(), Some("En curso"));
    assert_eq!(show.parent.as_deref(), Some("ACC-100"));
    assert!(show.body.contains("hola"), "{}", show.body);
    assert!(show.data_files.is_empty());
}

#[test]
fn live_leaves_the_body_empty_when_the_item_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "x", "Abierta", None, None);

    let show = show_live(dir.path(), "ACC-355", &provider).unwrap();

    assert_eq!(show.body, "");
}

#[test]
fn live_refuses_an_id_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    assert!(show_live(dir.path(), "../escape", &provider).is_err());
}

#[test]
fn local_reads_the_file_already_pulled_and_finds_it_by_id_alone() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: x\nstatus: En curso\n---\ncuerpo local\n");

    let show = show_local(dir.path(), "ACC-355").unwrap();

    assert_eq!(show.title, "x");
    assert_eq!(show.status.as_deref(), Some("En curso"));
    assert_eq!(show.body, "cuerpo local");
}

#[test]
fn local_shows_a_draft_that_never_synced() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "@un-borrador.task.md", "---\ntitle: un borrador\n---\n");

    let show = show_local(dir.path(), "@un-borrador").unwrap();

    assert_eq!(show.title, "un borrador");
    assert_eq!(show.status, None);
}

#[test]
fn local_lists_the_data_directory_when_it_exists() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-338.question.md", "---\ntitle: x\nstatus: Abierta\n---\n");
    std::fs::create_dir(dir.path().join("ACC-338_data")).unwrap();
    write(&dir.path().join("ACC-338_data"), "nota.md", "x");

    let show = show_local(dir.path(), "ACC-338").unwrap();

    assert_eq!(show.data_files, vec!["nota.md".to_string()]);
}

#[test]
fn local_refuses_an_id_with_no_matching_file() {
    let dir = tempfile::tempdir().unwrap();
    assert!(show_local(dir.path(), "ACC-999").is_err());
}
