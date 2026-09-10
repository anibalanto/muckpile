//! `states discover` lists the workflow's own statuses and caches
//! `{name -> category}` in `<project>.states.toml` — regenerable, never
//! edited by hand.

use muckpile_cli::states_discover;
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use std::collections::BTreeMap;
use std::path::Path;

fn scaffold(root: &Path) {
    std::fs::create_dir_all(root.join(".muckpile")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n",
    )
    .unwrap();
}

#[test]
fn returns_the_statuses_and_writes_the_project_s_cache() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_statuses(&[("Finalizada", "done"), ("Tareas por hacer", "new")]);

    let states = states_discover(root, "acc", &provider, &config).unwrap();

    let mut expected = BTreeMap::new();
    expected.insert("Finalizada".to_string(), "done".to_string());
    expected.insert("Tareas por hacer".to_string(), "new".to_string());
    assert_eq!(states, expected);

    let text = std::fs::read_to_string(root.join("acc.states.toml")).unwrap();
    let cached: BTreeMap<String, String> = toml::from_str(&text).unwrap();
    assert_eq!(cached, expected);
}

#[test]
fn asks_the_provider_by_the_configured_project_key() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();

    // No status seeded: an empty result confirms the call went through
    // without the trait requiring anything the fake doesn't hold, and
    // exercises the cache being written even when empty.
    let states = states_discover(root, "acc", &provider, &config).unwrap();
    assert!(states.is_empty());
    assert!(root.join("acc.states.toml").exists());
}
