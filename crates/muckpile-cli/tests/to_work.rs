//! `to-work` only assembles the view directory — no item fetched yet (that's
//! `pull`'s job, and it doesn't exist), no worktree (that's `code-work add`).

use muckpile_cli::to_work;
use std::path::Path;

fn scaffold(root: &Path) {
    std::fs::create_dir_all(root.join(".muckpile")).unwrap();
    std::fs::create_dir_all(root.join("base")).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::create_dir_all(root.join("to-work")).unwrap();
}

#[test]
fn creates_an_empty_view_at_the_project_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);

    let view = to_work(root, root, "ACC-355").unwrap();

    assert_eq!(view, root.join("to-work/ACC-355"));
    assert!(view.is_dir());
    assert_eq!(std::fs::read_dir(&view).unwrap().count(), 0, "no item, no _data — that's pull's job");
}

#[test]
fn creates_to_work_itself_when_the_project_does_not_have_it_yet() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".muckpile")).unwrap();

    let view = to_work(root, root, "ACC-355").unwrap();

    assert!(view.is_dir());
}

#[test]
fn refuses_to_run_anywhere_but_the_project_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);

    let err = to_work(root, &root.join("backlog/sprint"), "ACC-355").unwrap_err();
    assert!(err.to_string().contains("backlog/sprint"), "{err}");
    assert!(!root.join("to-work/ACC-355").exists());
}

#[test]
fn refuses_an_id_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);

    assert!(to_work(root, root, "../escape").is_err());
    assert!(to_work(root, root, "a/b").is_err());
}

#[test]
fn refuses_a_view_that_already_exists() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    std::fs::create_dir(root.join("to-work/ACC-355")).unwrap();

    let err = to_work(root, root, "ACC-355").unwrap_err();
    assert!(err.to_string().contains("ACC-355"), "{err}");
}
