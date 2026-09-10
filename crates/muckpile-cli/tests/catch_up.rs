//! After a command writes to the provider, the view it runs in catches up
//! with what the provider has now — the same as a `pull` of that item — so
//! the next `push` doesn't take the command's own write for a change on the
//! other side.

use muckpile_cli::{catch_up, push, title, CatchUp, PushResult};
use muckpile_core::project::ProjectConfig;
use muckpile_provider::fake::FakeProvider;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn config() -> ProjectConfig {
    let mut item_type = BTreeMap::new();
    item_type.insert("task".to_string(), "Tarea".into());
    ProjectConfig {
        provider: "jira-rest".into(),
        jira_base_url: "https://x.atlassian.net".into(),
        jira_project_key: "ACC".into(),
        jira_board_id: None,
        commit_prefix: "acc".into(),
        repos: BTreeMap::new(),
        item_type,
    }
}

fn git_view() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for args in [&["init", "-q"][..], &["config", "user.email", "t@t"], &["config", "user.name", "t"]] {
        assert!(Command::new("git").arg("-C").arg(dir.path()).args(args).status().unwrap().success());
    }
    dir
}

fn seed_pulled(view: &Path, provider: &FakeProvider) {
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, None);
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: vieja\nstatus: Abierta\n---\n").unwrap();
    muckpile_core::commit_paths(view, &["ACC-1.task.md"], "pull ACC-1").unwrap();
}

#[test]
fn the_view_catches_up_with_what_the_command_wrote() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    seed_pulled(view, &provider);
    title("ACC-1", "nueva", &provider).unwrap();

    let caught = catch_up(view, "ACC-1", &provider, &config()).unwrap();

    assert_eq!(caught, CatchUp::CaughtUp);
    let text = std::fs::read_to_string(view.join("ACC-1.task.md")).unwrap();
    assert!(text.contains("title: nueva"), "{text}");
    assert_eq!(push(view, &provider, &config()).unwrap()[0].result, PushResult::Unchanged, "the next push sees nothing new");
}

/// Catching up rewrites the file from the provider: over an edit nobody
/// committed, that would lose it. The provider is already written; the view
/// waits for the next pull.
#[test]
fn an_item_with_uncommitted_edits_is_left_behind() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    seed_pulled(view, &provider);
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: vieja\nstatus: Abierta\n---\nun cuerpo a medio escribir\n").unwrap();
    title("ACC-1", "nueva", &provider).unwrap();

    let caught = catch_up(view, "ACC-1", &provider, &config()).unwrap();

    assert!(matches!(caught, CatchUp::Behind(_)), "{caught:?}");
    let text = std::fs::read_to_string(view.join("ACC-1.task.md")).unwrap();
    assert!(text.contains("un cuerpo a medio escribir"), "{text}");
}

/// A tracked file in `_data/` edited by hand — a comment someone touched —
/// would be rewritten too: behind.
#[test]
fn an_edited_file_in_its_data_leaves_it_behind_too() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    seed_pulled(view, &provider);
    std::fs::create_dir_all(view.join("ACC-1_data/thread")).unwrap();
    std::fs::write(view.join("ACC-1_data/thread/42.md"), "---\nauthor: x\n---\nhola\n").unwrap();
    muckpile_core::commit_paths(view, &["ACC-1_data/thread/42.md"], "pull ACC-1").unwrap();
    std::fs::write(view.join("ACC-1_data/thread/42.md"), "---\nauthor: x\n---\nhola, editado\n").unwrap();

    assert!(matches!(catch_up(view, "ACC-1", &provider, &config()).unwrap(), CatchUp::Behind(_)));
}

/// A draft nobody committed in `files/` isn't at risk: what catching up
/// writes is only what comes from the provider, committed by name.
#[test]
fn a_local_draft_in_files_does_not_hold_it_back() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    seed_pulled(view, &provider);
    std::fs::create_dir_all(view.join("ACC-1_data/files")).unwrap();
    std::fs::write(view.join("ACC-1_data/files/borrador.md"), "sin commitear\n").unwrap();
    title("ACC-1", "nueva", &provider).unwrap();

    assert_eq!(catch_up(view, "ACC-1", &provider, &config()).unwrap(), CatchUp::CaughtUp);
    assert_eq!(std::fs::read_to_string(view.join("ACC-1_data/files/borrador.md")).unwrap(), "sin commitear\n");
}

#[test]
fn an_item_the_view_does_not_hold_is_not_brought_in() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-2", "Tarea", "otra", "Abierta", None, None);

    assert_eq!(catch_up(view, "ACC-2", &provider, &config()).unwrap(), CatchUp::NotHere);
    assert!(!view.join("ACC-2.task.md").exists());
}
