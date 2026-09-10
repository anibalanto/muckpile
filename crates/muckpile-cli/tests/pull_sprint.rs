//! `pull` of a sprint's view: the view holds exactly what the provider says
//! the sprint holds — every item in it comes, and one taken out of it goes —
//! in one record on the view's provider ref.

use muckpile_cli::{pull, pull_sprint, sprint_fetch, sprint_view_of, to_work};
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use std::path::Path;
use std::process::Command;

fn scaffold(root: &Path) {
    muckpile_core::ledger::init(root).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::create_dir_all(root.join("to-work")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\njira_board_id = 701\ncommit_prefix = \"acc\"\n\n[item_type]\ntask = \"Tarea\"\n",
    )
    .unwrap();
}

/// A project with the sprint `22 Las vistas` open, holding ACC-1 and ACC-2,
/// and its view already made by `sprint fetch`.
fn sprint(root: &Path) -> FakeProvider {
    scaffold(root);
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);
    provider.seed_item("ACC-1", "Tarea", "uno", "Abierta", None, None);
    provider.seed_item("ACC-2", "Tarea", "dos", "En curso", None, None);
    provider.seed_sprint_items(1, &["ACC-1", "ACC-2"]);
    sprint_fetch(root, &provider, &load_project_config(root).unwrap()).unwrap();
    provider
}

fn git(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(Command::new("git").arg("-C").arg(dir).args(args).output().unwrap().stdout).unwrap()
}

#[test]
fn brings_every_item_of_the_sprint_in_one_record() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let provider = sprint(root);
    let view = root.join("backlog/sprint/22_Las_vistas");

    let pulled = pull_sprint(root, &view, &provider, &load_project_config(root).unwrap()).unwrap();

    assert_eq!(pulled.items.len(), 2);
    assert!(view.join("ACC-1.task.md").exists() && view.join("ACC-2.task.md").exists());
    assert_eq!(git(&view, &["log", "-1", "--format=%s", "provider/backlog/sprint/22_Las_vistas"]).trim(), "pull backlog/sprint/22_Las_vistas");
    assert!(git(&view, &["status", "-sb"]).starts_with("## backlog/sprint/22_Las_vistas...provider/backlog/sprint/22_Las_vistas\n"));
}

/// Taken out of the sprint on the provider: the provider's record takes it
/// out of the view — its file, its ADF, its thread.
#[test]
fn an_item_taken_out_of_the_sprint_goes_from_its_view() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let provider = sprint(root);
    let config = load_project_config(root).unwrap();
    let view = root.join("backlog/sprint/22_Las_vistas");
    pull_sprint(root, &view, &provider, &config).unwrap();

    provider.seed_sprint_items(1, &["ACC-1"]);
    let pulled = pull_sprint(root, &view, &provider, &config).unwrap();

    assert_eq!(pulled.gone, vec!["ACC-2".to_string()]);
    assert!(!view.join("ACC-2.task.md").exists());
    assert_eq!(muckpile_core::ledger::provider_text(&view, ".provider/ACC-2.adf.json").unwrap(), None);
    assert!(view.join("ACC-1.task.md").exists());
}

#[test]
fn refuses_a_sprint_that_is_not_open() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let provider = sprint(root);
    let config = load_project_config(root).unwrap();
    let view = root.join("backlog/sprint/22_Las_vistas");
    std::fs::write(view.join("notas.md"), "algo\n").unwrap(); // so fetch keeps the folder
    provider.seed_sprints(&[("23 Las questions", "2026-08-05T00:00:00.000Z")]);

    let err = pull_sprint(root, &view, &provider, &config).unwrap_err();

    assert!(err.to_string().contains("22_Las_vistas"), "{err}");
}

/// The local check: whether this machine already has the item in another
/// view — asked of the disk right here, never of a shared record.
#[test]
fn says_which_other_view_already_holds_an_item() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let provider = sprint(root);
    let config = load_project_config(root).unwrap();
    let work = to_work(root, root, "ACC-1").unwrap();
    pull(root, &work, None, &provider, &config).unwrap();

    let pulled = pull_sprint(root, &root.join("backlog/sprint/22_Las_vistas"), &provider, &config).unwrap();

    let one = pulled.items.iter().find(|p| p.path.ends_with("ACC-1.task.md")).unwrap();
    assert_eq!(one.also_in, vec!["to-work/ACC-1".to_string()]);
    let two = pulled.items.iter().find(|p| p.path.ends_with("ACC-2.task.md")).unwrap();
    assert!(two.also_in.is_empty());
}

#[test]
fn a_work_view_s_pull_says_so_too() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let provider = sprint(root);
    let config = load_project_config(root).unwrap();
    pull_sprint(root, &root.join("backlog/sprint/22_Las_vistas"), &provider, &config).unwrap();
    let work = to_work(root, root, "ACC-1").unwrap();

    let pulled = pull(root, &work, None, &provider, &config).unwrap();

    assert_eq!(pulled.also_in, vec!["backlog/sprint/22_Las_vistas".to_string()]);
}

/// The sprint's view is named from the project's root, or it's the view
/// one stands in.
#[test]
fn the_sprint_view_is_the_one_named_or_the_one_stood_in() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    sprint(root);
    let view = root.join("backlog/sprint/22_Las_vistas");

    assert_eq!(sprint_view_of(root, root, Some("backlog/sprint/22_Las_vistas")), Some(view.clone()));
    assert_eq!(sprint_view_of(root, &view, None), Some(view.clone()));
    assert_eq!(sprint_view_of(root, root, None), None);
    assert_eq!(sprint_view_of(root, root, Some("ACC-1")), None, "an id is a work view's business");
}
