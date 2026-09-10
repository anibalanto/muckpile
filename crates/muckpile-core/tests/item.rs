//! Reading back what `pull` writes — `<id>.<type>.md`, frontmatter only,
//! for `list` to filter without asking the provider again.

use muckpile_core::item::{list_summaries, read_summary, ItemSummary};
use std::path::Path;

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn reads_title_status_and_id_and_type_from_the_filename() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: Vistas de trabajo\nstatus: En curso\n---\n");

    let summary = read_summary(&dir.path().join("ACC-355.task.md")).unwrap();

    assert_eq!(
        summary,
        ItemSummary { id: "ACC-355".into(), item_type: "task".into(), title: "Vistas de trabajo".into(), status: "En curso".into(), parent: None }
    );
}

#[test]
fn carries_the_parent_when_the_file_has_one() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: x\nstatus: Abierta\nparent: ACC-100\n---\n");

    let summary = read_summary(&dir.path().join("ACC-355.task.md")).unwrap();

    assert_eq!(summary.parent.as_deref(), Some("ACC-100"));
}

#[test]
fn refuses_a_file_with_no_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "no frontmatter here\n");

    assert!(read_summary(&dir.path().join("ACC-355.task.md")).is_err());
}

#[test]
fn list_summaries_only_picks_up_known_item_types_at_the_top_level() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "ACC-355.task.md", "---\ntitle: a\nstatus: Abierta\n---\n");
    write(root, "ACC-360.user-story.md", "---\ntitle: b\nstatus: Finalizada\n---\n");
    write(root, "README.md", "no es un item");
    std::fs::create_dir(root.join("code-work")).unwrap();
    write(&root.join("code-work"), "ACC-999.task.md", "---\ntitle: c\nstatus: Abierta\n---\n");

    let summaries = list_summaries(root).unwrap();

    let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["ACC-355", "ACC-360"], "README and anything under code-work/ must not appear");
}
