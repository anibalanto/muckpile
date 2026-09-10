//! Reading back what `pull` writes — `<id>.<type>.md`, frontmatter only,
//! for `list` to filter without asking the provider again.

use muckpile_core::item::{list_summaries, read_full, read_summary, FullItem, ItemSummary};
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

/// A freshly `new`-ed item has no status yet — nothing to compare against
/// the provider, so `list`/`status` skip it rather than fail parsing it.
#[test]
fn list_summaries_skips_a_slug_that_has_not_synced_yet() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "ACC-355.task.md", "---\ntitle: a\nstatus: Abierta\n---\n");
    write(root, "@un-borrador.task.md", "---\ntitle: un borrador\n---\n");

    let summaries = list_summaries(root).unwrap();

    let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["ACC-355"]);
}

#[test]
fn read_full_carries_the_body_and_a_present_status() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\ntitle: x\nstatus: En curso\nparent: ACC-100\n---\nun parrafo\n\notro parrafo\n");

    let full = read_full(&dir.path().join("ACC-355.task.md")).unwrap();

    assert_eq!(
        full,
        FullItem {
            id: "ACC-355".into(),
            item_type: "task".into(),
            title: "x".into(),
            status: Some("En curso".into()),
            parent: Some("ACC-100".into()),
            body: "un parrafo\n\notro parrafo".into(),
        }
    );
}

/// A draft `new` wrote has no status yet — `show --local` still has
/// something to show.
#[test]
fn read_full_tolerates_a_draft_with_no_status() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "@un-borrador.task.md", "---\ntitle: un borrador\n---\n");

    let full = read_full(&dir.path().join("@un-borrador.task.md")).unwrap();

    assert_eq!(full.status, None);
    assert_eq!(full.title, "un borrador");
}

#[test]
fn read_full_still_requires_a_title() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-355.task.md", "---\nstatus: Abierta\n---\ncuerpo\n");

    assert!(read_full(&dir.path().join("ACC-355.task.md")).is_err());
}
