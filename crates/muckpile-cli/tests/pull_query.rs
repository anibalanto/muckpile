//! `pull` of a named query: its view, `query/<name>/`, holds
//! exactly what the query returns — the query declared in `muckpile.toml`,
//! or given with `--query` for one pull, never saved.

use muckpile_cli::{pull_query, query_view_of};
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use std::path::Path;

fn scaffold(root: &Path, queries: &str) {
    muckpile_core::ledger::init(root).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        format!("provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n\n[item_type]\ntask = \"Tarea\"\n\n[queries]\n{queries}"),
    )
    .unwrap();
}

fn provider() -> FakeProvider {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "uno", "Abierta", None, None);
    provider.seed_item("ACC-2", "Tarea", "dos", "Abierta", None, None);
    provider.seed_query("project = ACC AND sprint is empty", &["ACC-1", "ACC-2"]);
    provider
}

/// No command makes a query's view ahead of time: the first pull does.
#[test]
fn a_declared_query_s_view_is_made_and_holds_what_it_returns() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root, "sin-sprint = \"project = ACC AND sprint is empty\"\n");
    let config = load_project_config(root).unwrap();
    let view = root.join("query/sin-sprint");

    let pulled = pull_query(root, &view, None, &provider(), &config).unwrap();

    assert_eq!(pulled.items.len(), 2);
    assert!(view.join("ACC-1.task.md").exists() && view.join("ACC-2.task.md").exists());
    assert_eq!(muckpile_core::ledger::branch(&view).unwrap(), "query/sin-sprint");
}

#[test]
fn what_stops_matching_goes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root, "sin-sprint = \"project = ACC AND sprint is empty\"\n");
    let config = load_project_config(root).unwrap();
    let view = root.join("query/sin-sprint");
    let provider = provider();
    pull_query(root, &view, None, &provider, &config).unwrap();

    provider.seed_query("project = ACC AND sprint is empty", &["ACC-2"]);
    let pulled = pull_query(root, &view, None, &provider, &config).unwrap();

    assert_eq!(pulled.gone, vec!["ACC-1".to_string()]);
    assert!(!view.join("ACC-1.task.md").exists());
}

/// `--query` is for that one pull: it isn't saved, so the next one without
/// it goes back to what `muckpile.toml` declares — or, with nothing
/// declared, refuses.
#[test]
fn a_query_given_for_one_pull_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root, "");
    let config = load_project_config(root).unwrap();
    let view = root.join("query/a-mano");
    let provider = provider();
    provider.seed_query("key = ACC-1", &["ACC-1"]);

    let pulled = pull_query(root, &view, Some("key = ACC-1"), &provider, &config).unwrap();
    assert_eq!(pulled.items.len(), 1);

    let err = pull_query(root, &view, None, &provider, &config).unwrap_err();
    assert!(err.to_string().contains("a-mano") && err.to_string().contains("muckpile.toml"), "{err}");
}

#[test]
fn the_query_view_is_the_one_named_or_the_one_stood_in() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root, "");
    let view = root.join("query/sin-sprint");

    assert_eq!(query_view_of(root, root, Some("query/sin-sprint")), Some(view.clone()), "it may not exist yet");
    std::fs::create_dir_all(&view).unwrap();
    assert_eq!(query_view_of(root, &view, None), Some(view.clone()));
    assert_eq!(query_view_of(root, root, Some("backlog/sprint/22_Las_vistas")), None);
    assert_eq!(query_view_of(root, root, None), None);
}
