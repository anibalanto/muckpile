//! `to-work` only assembles the view — a worktree of the project's ledger,
//! on its own branch — with no item fetched yet (that's `pull`'s job) and no
//! code worktree (that's `code-work add`).

use muckpile_cli::to_work;
use std::path::Path;

fn scaffold(root: &Path) {
    muckpile_core::ledger::init(root).unwrap();
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
    let entries: Vec<String> = std::fs::read_dir(&view).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(entries, vec![".git".to_string()], "no item, no _data — that's pull's job");
    let branch = std::process::Command::new("git").arg("-C").arg(&view).args(["symbolic-ref", "--short", "HEAD"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "to-work/ACC-355", "a worktree of the ledger, on the view's own branch");
}

#[test]
fn creates_to_work_itself_when_the_project_does_not_have_it_yet() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    muckpile_core::ledger::init(root).unwrap();

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

fn config_file(root: &Path) {
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n\n[item_type]\ntask = \"Tarea\"\n",
    )
    .unwrap();
}

/// By default the view comes with its item: `to-work` is a view and a pull.
#[test]
fn brings_the_item_into_its_new_view() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    config_file(root);
    let config = muckpile_core::project::load_project_config(root).unwrap();
    let provider = muckpile_provider::fake::FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", None, None);

    let pulled = muckpile_cli::to_work_and_pull(root, root, "ACC-355", &provider, &config).unwrap();

    assert_eq!(pulled.path, root.join("to-work/ACC-355/ACC-355.task.md"));
    assert!(pulled.path.exists());
}

/// A view whose item couldn't be brought doesn't stay behind, empty: a
/// mistyped id leaves nothing to clean up.
#[test]
fn an_item_that_cannot_be_brought_leaves_no_view() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    config_file(root);
    let config = muckpile_core::project::load_project_config(root).unwrap();
    let provider = muckpile_provider::fake::FakeProvider::new();

    assert!(muckpile_cli::to_work_and_pull(root, root, "ACC-999", &provider, &config).is_err());
    assert!(!root.join("to-work/ACC-999").exists());
}
