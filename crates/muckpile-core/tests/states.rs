//! `<project>.states.toml` — the cache `states discover` writes: {name ->
//! category}, regenerable, never edited by hand (decision 8).

use muckpile_core::states::{read_states_cache, write_states_cache};
use std::collections::BTreeMap;

#[test]
fn writes_names_and_categories_as_a_flat_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("acc.states.toml");
    let mut states = BTreeMap::new();
    states.insert("Finalizada".to_string(), "done".to_string());
    states.insert("Tareas por hacer".to_string(), "new".to_string());

    write_states_cache(&path, &states).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let back: BTreeMap<String, String> = toml::from_str(&text).unwrap();
    assert_eq!(back, states);
}

#[test]
fn regenerating_replaces_whatever_was_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("acc.states.toml");
    let mut first = BTreeMap::new();
    first.insert("Vieja".to_string(), "new".to_string());
    write_states_cache(&path, &first).unwrap();

    let mut second = BTreeMap::new();
    second.insert("Finalizada".to_string(), "done".to_string());
    write_states_cache(&path, &second).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let back: BTreeMap<String, String> = toml::from_str(&text).unwrap();
    assert_eq!(back, second);
}

#[test]
fn reads_back_what_was_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("acc.states.toml");
    let mut states = BTreeMap::new();
    states.insert("Finalizada".to_string(), "done".to_string());
    write_states_cache(&path, &states).unwrap();

    assert_eq!(read_states_cache(&path).unwrap(), states);
}

#[test]
fn refuses_a_cache_that_was_never_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("acc.states.toml");

    let err = read_states_cache(&path).unwrap_err();
    assert!(err.to_string().contains("states discover"), "{err}");
}
