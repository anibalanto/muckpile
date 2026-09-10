//! `new` writes `@<slug>.<type>.md` local — no network, no provider. The
//! slug comes from slugifying the title; `--blocks` is the only relation it
//! can declare at creation (the one a `question` exists to declare), and
//! `--parent` the item it hangs from.

use muckpile_cli::new;
use std::path::Path;

#[test]
fn writes_the_slug_derived_from_the_title() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "task", "Arreglar el hook que no arranca", None, None).unwrap();

    assert_eq!(path, dir.path().join("@arreglar-el-hook-que-no-arranca.task.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: Arreglar el hook que no arranca\n---\n");
}

#[test]
fn a_question_can_declare_what_it_blocks() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "question", "¿el rol se hereda de la capa de arriba?", None, Some("ACC-229")).unwrap();

    assert_eq!(path, dir.path().join("@el-rol-se-hereda-de-la-capa-de-arriba.question.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: ¿el rol se hereda de la capa de arriba?\nrelation.blocks: ACC-229\n---\n");
}

#[test]
fn a_draft_can_declare_its_parent() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "task", "Vistas de trabajo", Some("ACC-339"), None).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: Vistas de trabajo\nparent: ACC-339\n---\n");
}

#[test]
fn the_parent_goes_before_the_relations() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "question", "¿se hereda?", Some("ACC-339"), Some("ACC-229")).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: ¿se hereda?\nparent: ACC-339\nrelation.blocks: ACC-229\n---\n");
}

#[test]
fn the_parent_can_be_another_draft() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "task", "La tarea", Some("@la-epica"), None).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: La tarea\nparent: @la-epica\n---\n");
}

#[test]
fn refuses_a_parent_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();

    assert!(new(dir.path(), "task", "algo", Some("../escape"), None).is_err());
    assert!(!dir.path().join("@algo.task.md").exists());
}

#[test]
fn refuses_an_unknown_type() {
    let dir = tempfile::tempdir().unwrap();
    assert!(new(dir.path(), "bug", "algo", None, None).is_err());
}

#[test]
fn refuses_a_blocks_target_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    assert!(new(dir.path(), "question", "algo", None, Some("../escape")).is_err());
}

#[test]
fn refuses_to_overwrite_a_slug_that_already_exists() {
    let dir = tempfile::tempdir().unwrap();
    new(dir.path(), "task", "Mismo título", None, None).unwrap();

    let err = new(dir.path(), "task", "Mismo título", None, None).unwrap_err();
    assert!(err.to_string().contains("mismo-titulo"), "{err}");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

#[test]
fn two_different_titles_never_collide() {
    let dir = tempfile::tempdir().unwrap();
    new(dir.path(), "task", "Primero", None, None).unwrap();
    new(dir.path(), "task", "Segundo", None, None).unwrap();

    assert!(read(dir.path(), "@primero.task.md").contains("Primero"));
    assert!(read(dir.path(), "@segundo.task.md").contains("Segundo"));
}
