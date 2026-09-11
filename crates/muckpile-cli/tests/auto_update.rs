//! `auto_update`: every command that creates or edits on the provider goes
//! through a person at a terminal, unless the project lets it write on its
//! own. Reading never asks.

use muckpile_cli::{approve_update, attach, link, parent, push, title, transition, unlink, Planned, PushResult};
use muckpile_core::project::ProjectConfig;
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::{Provider, Transition};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn config(auto_update: bool) -> ProjectConfig {
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
        queries: BTreeMap::new(),
        auto_update,
        auto_comment: false,
    }
}

/// An agent's shell: there's no terminal to show the phrase on.
fn no_terminal(_: &str) -> anyhow::Result<String> {
    anyhow::bail!("no terminal")
}

/// No approval: what a command gets when the person isn't there.
fn refused() -> anyhow::Result<()> {
    anyhow::bail!("refused")
}

#[test]
fn with_auto_update_it_goes_without_asking() {
    approve_update(&config(true), |_| panic!("asked for a phrase with auto_update = true")).unwrap();
}

#[test]
fn without_auto_update_a_person_who_retypes_the_phrase_lets_it_go() {
    let asked = Cell::new(false);

    approve_update(&config(false), |phrase| {
        asked.set(true);
        Ok(format!("{phrase}\n"))
    })
    .unwrap();

    assert!(asked.get());
}

#[test]
fn without_auto_update_and_with_no_terminal_it_is_refused() {
    let err = approve_update(&config(false), no_terminal).unwrap_err();
    assert!(err.to_string().contains("terminal"), "{err}");
}

#[test]
fn without_auto_update_anything_but_the_phrase_is_refused() {
    assert!(approve_update(&config(false), |_| Ok("otra-cosa".into())).is_err());
}

#[test]
fn a_title_not_approved_is_not_written() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, None);

    assert!(title("ACC-1", "nueva", &provider, refused).is_err());

    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("vieja"));
}

#[test]
fn a_parent_not_approved_is_not_written() {
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    provider.seed_item("ACC-2", "Épica", "y", "Abierta", None, None);

    assert!(parent("ACC-1", "ACC-2", &provider, refused).is_err());

    assert_eq!(provider.parent_of("ACC-1"), None);
}

#[test]
fn a_transition_not_approved_is_not_fired() {
    let provider = FakeProvider::new();
    provider.seed("ACC-1", "Abierta", vec![Transition { id: "31".into(), name: "Cerrar".into(), to: "Finalizada".into() }]);

    assert!(transition("ACC-1", "Finalizada", &provider, refused).is_err());

    assert_eq!(provider.status_of("ACC-1").as_deref(), Some("Abierta"));
}

#[test]
fn a_link_or_an_unlink_not_approved_is_not_written() {
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    provider.seed_item("ACC-2", "Tarea", "y", "Abierta", None, None);

    assert!(link("ACC-1", "blocks", "ACC-2", &provider, refused).is_err());
    assert!(provider.links_created().is_empty());

    link("ACC-1", "blocks", "ACC-2", &provider, || Ok(())).unwrap();
    assert!(unlink("ACC-1", "blocks", "ACC-2", &provider, refused).is_err());
    assert_eq!(provider.links_created().len(), 1);
}

#[test]
fn an_attachment_not_approved_is_not_uploaded() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("captura.png");
    std::fs::write(&file, b"png").unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);

    assert!(attach("ACC-1", &file, &provider, refused).is_err());

    assert!(provider.item("ACC-1").unwrap().attachments.is_empty());
}

/// A mistyped command is refused before anyone is asked for a phrase.
#[test]
fn a_command_that_would_fail_anyway_asks_nobody() {
    let provider = FakeProvider::new();
    let asked = || -> anyhow::Result<()> { panic!("asked for a phrase before checking the arguments") };

    assert!(title("../x", "nueva", &provider, asked).is_err());
    assert!(title("ACC-1", " ", &provider, asked).is_err());
    assert!(parent("ACC-1", "ACC-1", &provider, asked).is_err());
    assert!(transition("../x", "Finalizada", &provider, asked).is_err());
    assert!(link("../x", "blocks", "ACC-2", &provider, asked).is_err());
    assert!(unlink("ACC-1", "blocks", "../x", &provider, asked).is_err());
}

fn git_view() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("acc");
    std::fs::create_dir_all(&root).unwrap();
    muckpile_core::ledger::init(&root).unwrap();
    let view = muckpile_core::ledger::open_view(&root, "to-work/ACC-1").unwrap();
    (dir, view)
}

fn commit_all(view: &Path) {
    for args in [&["add", "-A"][..], &["-c", "user.name=Ana", "-c", "user.email=ana@x", "commit", "-qm", "edit"]] {
        assert!(Command::new("git").arg("-C").arg(view).args(args).status().unwrap().success());
    }
}

/// A view with one pulled item whose body was edited, one committed draft,
/// and one draft nobody committed.
fn view_with_writes(provider: &FakeProvider) -> (tempfile::TempDir, std::path::PathBuf) {
    let (dir, view) = git_view();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(adf));
    let root = view.parent().unwrap().parent().unwrap();
    muckpile_cli::pull(root, &view, Some("ACC-1"), provider, &config(false)).unwrap();
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Abierta\n---\neditado\n").unwrap();
    std::fs::write(view.join("@nueva.task.md"), "---\ntitle: Una tarea nueva\n---\n").unwrap();
    commit_all(&view);
    std::fs::write(view.join("@seguimiento.task.md"), "---\ntitle: De seguimiento\n---\n").unwrap();
    (dir, view)
}

/// `push` says everything it's about to create and edit, and asks once for
/// all of it — never for what stays local.
#[test]
fn push_shows_what_it_will_write_and_asks_once() {
    let provider = FakeProvider::new();
    let (_dir, view) = view_with_writes(&provider);
    provider.queue_create("ACC-403", "Tareas por hacer");
    let mut shown = Vec::new();

    let outcomes = push(&view, &provider, &config(false), |plan| {
        shown = plan.to_vec();
        Ok(())
    })
    .unwrap();

    assert_eq!(shown, vec![Planned::Draft("@nueva".into()), Planned::Body("ACC-1".into())]);
    assert_eq!(provider.title_of("ACC-403").as_deref(), Some("Una tarea nueva"));
    assert!(outcomes.iter().any(|o| o.id == "@seguimiento" && o.result == PushResult::Local), "{outcomes:?}");
}

/// Refused, `push` writes nothing at all: no draft created, no body sent.
#[test]
fn push_not_approved_writes_nothing() {
    let provider = FakeProvider::new();
    let (_dir, view) = view_with_writes(&provider);
    // No queued key: `create_item` would panic on an empty queue.

    let err = push(&view, &provider, &config(false), |_| approve_update(&config(false), no_terminal)).unwrap_err();

    assert!(err.to_string().contains("terminal"), "{err}");
    let body = provider.body_adf_of("ACC-1").unwrap();
    assert!(body.contains("original"), "{body}");
    assert!(view.join("@nueva.task.md").exists());
}

/// Nothing to write, nobody to ask: a draft that stays local isn't a write.
#[test]
fn push_with_nothing_to_write_asks_nobody() {
    let provider = FakeProvider::new();
    let (_dir, view) = git_view();
    std::fs::write(view.join("@seguimiento.task.md"), "---\ntitle: De seguimiento\n---\n").unwrap();

    let outcomes = push(&view, &provider, &config(false), |_| panic!("asked with nothing to write")).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Local);
}

/// A header edited by hand isn't something `push` will write, so it isn't
/// shown as one.
#[test]
fn a_header_edit_is_not_in_what_push_will_write() {
    let provider = FakeProvider::new();
    let (_dir, view) = git_view();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    let root = view.parent().unwrap().parent().unwrap();
    muckpile_cli::pull(root, &view, Some("ACC-1"), &provider, &config(false)).unwrap();
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: otro\nstatus: Abierta\n---\n").unwrap();
    commit_all(&view);

    let outcomes = push(&view, &provider, &config(false), |_| panic!("asked for a header clash")).unwrap();

    assert!(matches!(outcomes[0].result, PushResult::HeaderClash(_)), "{outcomes:?}");
}
