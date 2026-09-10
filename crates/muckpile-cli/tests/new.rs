//! `new` writes `@<slug>.<type>.md` local — no network, no provider. The
//! slug comes from slugifying the title; `--blocks` is the only relation it
//! can declare at creation (decision 7's `question`).

use muckpile_cli::new;
use std::path::Path;

#[test]
fn writes_the_slug_derived_from_the_title() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "task", "Arreglar el hook que no arranca", None).unwrap();

    assert_eq!(path, dir.path().join("@arreglar-el-hook-que-no-arranca.task.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: Arreglar el hook que no arranca\n---\n");
}

#[test]
fn a_question_can_declare_what_it_blocks() {
    let dir = tempfile::tempdir().unwrap();

    let path = new(dir.path(), "question", "¿el rol se hereda de la capa de arriba?", Some("ACC-229")).unwrap();

    assert_eq!(path, dir.path().join("@el-rol-se-hereda-de-la-capa-de-arriba.question.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "---\ntitle: ¿el rol se hereda de la capa de arriba?\nrelation.blocks: ACC-229\n---\n");
}

#[test]
fn refuses_an_unknown_type() {
    let dir = tempfile::tempdir().unwrap();
    assert!(new(dir.path(), "bug", "algo", None).is_err());
}

#[test]
fn refuses_a_blocks_target_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    assert!(new(dir.path(), "question", "algo", Some("../escape")).is_err());
}

#[test]
fn refuses_to_overwrite_a_slug_that_already_exists() {
    let dir = tempfile::tempdir().unwrap();
    new(dir.path(), "task", "Mismo título", None).unwrap();

    let err = new(dir.path(), "task", "Mismo título", None).unwrap_err();
    assert!(err.to_string().contains("mismo-titulo"), "{err}");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

#[test]
fn two_different_titles_never_collide() {
    let dir = tempfile::tempdir().unwrap();
    new(dir.path(), "task", "Primero", None).unwrap();
    new(dir.path(), "task", "Segundo", None).unwrap();

    assert!(read(dir.path(), "@primero.task.md").contains("Primero"));
    assert!(read(dir.path(), "@segundo.task.md").contains("Segundo"));
}
