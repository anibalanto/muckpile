//! `list` filters the items already pulled into a view — by state (the
//! provider's own string), category (from the cache `states discover`
//! wrote), or parent.

use muckpile_cli::{list, ListFilter};
use std::collections::BTreeMap;
use std::path::Path;

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

fn scaffold(dir: &Path) {
    write(dir, "ACC-355.task.md", "---\ntitle: Vistas de trabajo\nstatus: Finalizada\nparent: ACC-100\n---\n");
    write(dir, "ACC-360.task.md", "---\ntitle: Otra tarea\nstatus: En curso\n---\n");
    write(dir, "ACC-361.task.md", "---\ntitle: Tercera\nstatus: Tareas por hacer\nparent: ACC-100\n---\n");
}

fn categories() -> BTreeMap<String, String> {
    let mut c = BTreeMap::new();
    c.insert("Finalizada".to_string(), "done".to_string());
    c.insert("En curso".to_string(), "indeterminate".to_string());
    c.insert("Tareas por hacer".to_string(), "new".to_string());
    c
}

#[test]
fn no_filter_returns_everything_sorted_by_id() {
    let dir = tempfile::tempdir().unwrap();
    scaffold(dir.path());

    let items = list(dir.path(), &ListFilter::default(), &BTreeMap::new()).unwrap();

    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["ACC-355", "ACC-360", "ACC-361"]);
}

#[test]
fn filters_by_the_provider_s_own_state_string() {
    let dir = tempfile::tempdir().unwrap();
    scaffold(dir.path());

    let filter = ListFilter { state: Some("En curso"), ..Default::default() };
    let items = list(dir.path(), &filter, &BTreeMap::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "ACC-360");
}

#[test]
fn filters_by_category_using_the_states_cache() {
    let dir = tempfile::tempdir().unwrap();
    scaffold(dir.path());

    let filter = ListFilter { category: Some("done"), ..Default::default() };
    let items = list(dir.path(), &filter, &categories()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "ACC-355");
}

#[test]
fn an_uncached_status_never_matches_a_category_filter() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ACC-1.task.md", "---\ntitle: x\nstatus: Estado raro\n---\n");

    let filter = ListFilter { category: Some("done"), ..Default::default() };
    let items = list(dir.path(), &filter, &categories()).unwrap();

    assert!(items.is_empty(), "a status the cache doesn't know can't be claimed to match any category");
}

#[test]
fn filters_by_parent() {
    let dir = tempfile::tempdir().unwrap();
    scaffold(dir.path());

    let filter = ListFilter { parent: Some("ACC-100"), ..Default::default() };
    let items = list(dir.path(), &filter, &BTreeMap::new()).unwrap();

    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["ACC-355", "ACC-361"]);
}

#[test]
fn filters_combine_as_and() {
    let dir = tempfile::tempdir().unwrap();
    scaffold(dir.path());

    let filter = ListFilter { parent: Some("ACC-100"), category: Some("new"), ..Default::default() };
    let items = list(dir.path(), &filter, &categories()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "ACC-361");
}
