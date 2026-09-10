//! `sprint fetch`: one empty folder per open sprint under `backlog/sprint/`,
//! named by slugifying the provider's own sprint name (spaces to `_`,
//! nothing else touched). Never deletes a folder with anything in it —
//! only ones it left empty whose sprint isn't open any more.

use muckpile_cli::sprint_fetch;
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use std::path::Path;

fn scaffold(root: &Path) {
    muckpile_core::ledger::init(root).unwrap();
    std::fs::create_dir_all(root.join("base")).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::create_dir_all(root.join("to-work")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\njira_board_id = 701\ncommit_prefix = \"acc\"\n",
    )
    .unwrap();
}

#[test]
fn creates_one_empty_folder_per_open_sprint_slugified() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z"), ("23 Las questions", "2026-08-05T00:00:00.000Z")]);

    let result = sprint_fetch(root, &provider, &config).unwrap();

    assert_eq!(result.open, 2);
    assert_eq!(result.created, vec!["22_Las_vistas".to_string(), "23_Las_questions".to_string()]);
    assert!(root.join("backlog/sprint/22_Las_vistas").is_dir());
    assert!(root.join("backlog/sprint/23_Las_questions").is_dir());
    let branch = std::process::Command::new("git").arg("-C").arg(root.join("backlog/sprint/22_Las_vistas")).args(["symbolic-ref", "--short", "HEAD"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "backlog/sprint/22_Las_vistas", "each sprint is a view of the ledger");
}

#[test]
fn a_second_run_does_not_recreate_or_report_a_folder_that_is_already_there() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);

    sprint_fetch(root, &provider, &config).unwrap();
    let result = sprint_fetch(root, &provider, &config).unwrap();

    assert!(result.created.is_empty());
    assert_eq!(result.open, 1);
}

#[test]
fn never_deletes_a_folder_that_has_something_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);
    sprint_fetch(root, &provider, &config).unwrap();
    std::fs::write(root.join("backlog/sprint/22_Las_vistas/ACC-1.task.md"), "real work").unwrap();

    // The sprint closes: fetch no longer lists it.
    provider.seed_sprints(&[("23 Las questions", "2026-08-05T00:00:00.000Z")]);
    let result = sprint_fetch(root, &provider, &config).unwrap();

    assert!(root.join("backlog/sprint/22_Las_vistas").is_dir(), "a populated folder must survive");
    assert!(!result.removed.contains(&"22_Las_vistas".to_string()));
}

#[test]
fn removes_a_folder_it_left_empty_once_its_sprint_closes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);
    sprint_fetch(root, &provider, &config).unwrap();

    provider.seed_sprints(&[("23 Las questions", "2026-08-05T00:00:00.000Z")]);
    let result = sprint_fetch(root, &provider, &config).unwrap();

    assert!(!root.join("backlog/sprint/22_Las_vistas").exists());
    assert_eq!(result.removed, vec!["22_Las_vistas".to_string()]);
}

/// Measured against the real ACC board (701): several open sprints have a
/// `name` the provider itself already truncated to 29 characters with a
/// trailing `…` — confirmed against two different Jira endpoints, so it's
/// the provider's own data, not an artifact of one listing call.
#[test]
fn a_provider_truncated_name_gets_its_ellipsis_replaced_by_the_sprint_s_creation_date() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("11 El worklist se sincroniza…", "2026-09-05T19:13:14.128Z")]);

    let result = sprint_fetch(root, &provider, &config).unwrap();

    assert_eq!(result.created, vec!["11_El_worklist_se_sincroniza_2026-09-05".to_string()]);
}

#[test]
fn a_name_the_provider_did_not_truncate_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);

    let result = sprint_fetch(root, &provider, &config).unwrap();

    assert_eq!(result.created, vec!["22_Las_vistas".to_string()]);
}

#[test]
fn refuses_without_a_configured_board_id() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    muckpile_core::ledger::init(root).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n",
    )
    .unwrap();
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();

    let err = sprint_fetch(root, &provider, &config).unwrap_err();
    assert!(err.to_string().contains("jira_board_id"), "{err}");
}
