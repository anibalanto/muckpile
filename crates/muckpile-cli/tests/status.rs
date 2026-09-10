//! `status` compares each item already pulled into a view against the
//! provider, live, without writing anything back.

use muckpile_cli::status;
use muckpile_provider::fake::FakeProvider;
use std::path::Path;

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn reports_no_change_when_local_matches_the_provider() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: Vistas de trabajo\nstatus: En curso\n---\n");
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);

    let statuses = status(dir.path(), &provider).unwrap();

    assert_eq!(statuses.len(), 1);
    assert!(!statuses[0].changed, "{:?}", statuses[0]);
}

#[test]
fn reports_a_status_that_moved_on_the_provider_side() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: x\nstatus: En curso\n---\n");
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "x", "Finalizada", None, None);

    let statuses = status(dir.path(), &provider).unwrap();

    assert!(statuses[0].changed);
    assert_eq!(statuses[0].local_status, "En curso");
    assert_eq!(statuses[0].remote_status, "Finalizada");
}

#[test]
fn reports_a_title_rewritten_on_the_provider_side() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: vieja\nstatus: Abierta\n---\n");
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "nueva", "Abierta", None, None);

    let statuses = status(dir.path(), &provider).unwrap();

    assert!(statuses[0].changed);
    assert_eq!(statuses[0].local_title, "vieja");
    assert_eq!(statuses[0].remote_title, "nueva");
}

#[test]
fn reports_a_parent_gained_on_the_provider_side() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: x\nstatus: Abierta\n---\n");
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "x", "Abierta", Some("ACC-100"), None);

    let statuses = status(dir.path(), &provider).unwrap();

    assert!(statuses[0].changed);
    assert_eq!(statuses[0].local_parent, None);
    assert_eq!(statuses[0].remote_parent.as_deref(), Some("ACC-100"));
}

#[test]
fn covers_every_item_in_the_view() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: a\nstatus: Abierta\n---\n");
    write(dir.path(), "ACC-360.task.md", "---\ntitle: b\nstatus: Abierta\n---\n");
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "a", "Abierta", None, None);
    provider.seed_item("ACC-360", "Tarea", "b", "Finalizada", None, None);

    let statuses = status(dir.path(), &provider).unwrap();

    let ids: Vec<&str> = statuses.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["ACC-355", "ACC-360"]);
    assert!(!statuses[0].changed);
    assert!(statuses[1].changed);
}
