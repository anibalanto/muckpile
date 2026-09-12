//! `sprint fetch`: one empty folder per open sprint under `backlog/sprint/`,
//! named by slugifying the provider's own sprint name (spaces to `_`,
//! nothing else touched). Never deletes a folder with anything in it —
//! only ones it left empty whose sprint isn't open any more.

use muckpile_cli::sprint_fetch;
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use std::path::Path;

/// La config de un proyecto con board, para lo que no lee del disco.
fn config() -> muckpile_core::project::ProjectConfig {
    muckpile_core::project::ProjectConfig {
        provider: "jira-rest".into(),
        jira_base_url: "https://x.atlassian.net".into(),
        jira_project_key: "ACC".into(),
        jira_board_id: Some(701),
        commit_prefix: "acc".into(),
        repos: Default::default(),
        item_type: Default::default(),
        queries: Default::default(),
        auto_update: false,
        auto_comment: false,
    }
}

/// La persona aprobó la escritura, o el proyecto escribe solo.
fn approved() -> anyhow::Result<()> {
    Ok(())
}

/// Nadie la aprobó.
fn refused() -> anyhow::Result<()> {
    anyhow::bail!("sin aprobar")
}

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

/// `sprint create` manda el nombre entero al board del proyecto y devuelve el
/// id que el proveedor le dio.
#[test]
fn sprint_create_sends_the_name_whole_and_returns_the_id() {
    let provider = FakeProvider::new();
    provider.queue_sprint(44);

    let id = muckpile_cli::sprint_create("22 Después de muckpile", &provider, &config(), approved).unwrap();

    assert_eq!(id, 44);
    assert_eq!(provider.sprints_created(), vec![(701, "22 Después de muckpile".to_string())]);
}

/// Sin aprobación no se escribe nada: es una escritura en el proveedor como
/// cualquier otra.
#[test]
fn a_sprint_not_approved_is_not_created() {
    let provider = FakeProvider::new();

    assert!(muckpile_cli::sprint_create("22 Después de muckpile", &provider, &config(), refused).is_err());

    assert!(provider.sprints_created().is_empty());
}

#[test]
fn a_sprint_without_a_name_is_refused_before_asking_anyone() {
    let provider = FakeProvider::new();
    let asked = || -> anyhow::Result<()> { panic!("pidió la frase antes de mirar el nombre") };

    assert!(muckpile_cli::sprint_create("   ", &provider, &config(), asked).is_err());
    assert!(provider.sprints_created().is_empty());
}

/// El board sale de `muckpile.toml`: sin él, no hay dónde crearlo.
#[test]
fn a_project_without_a_board_cannot_create_a_sprint() {
    let provider = FakeProvider::new();
    let mut config = config();
    config.jira_board_id = None;

    assert!(muckpile_cli::sprint_create("22 Después de muckpile", &provider, &config, approved).is_err());
    assert!(provider.sprints_created().is_empty());
}

/// El sprint nuevo queda futuro, así que `sprint fetch` —que trae los
/// abiertos— no le hace vista.
#[test]
fn a_created_sprint_has_no_view_until_it_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    muckpile_core::ledger::init(root).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    let provider = FakeProvider::new();
    provider.queue_sprint(44);

    muckpile_cli::sprint_create("22 Después de muckpile", &provider, &config(), approved).unwrap();
    let fetched = muckpile_cli::sprint_fetch(root, &provider, &config()).unwrap();

    assert_eq!(fetched.open, 0);
    assert!(fetched.created.is_empty());
    assert!(!root.join("backlog/sprint/22_Después_de_muckpile").exists());
}

/// `sprint add` con el id del sprint: los ítems se mueven, y ningún archivo
/// de la vista cambia.
#[test]
fn sprint_add_moves_the_items_by_sprint_id() {
    let provider = FakeProvider::new();

    muckpile_cli::sprint_add("6533", &["ACC-1".into(), "ACC-2".into()], &provider, &config(), approved).unwrap();

    assert_eq!(provider.sprint_additions(), vec![(6533, vec!["ACC-1".to_string(), "ACC-2".to_string()])]);
}

/// Y con el slug de la vista de un sprint abierto, que es como se lo nombra
/// todos los días.
#[test]
fn sprint_add_takes_the_slug_of_an_open_sprint() {
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);

    muckpile_cli::sprint_add("22_Las_vistas", &["ACC-1".into()], &provider, &config(), approved).unwrap();

    assert_eq!(provider.sprint_additions(), vec![(1, vec!["ACC-1".to_string()])]);
}

#[test]
fn a_slug_that_is_no_open_sprint_is_refused() {
    let provider = FakeProvider::new();
    provider.seed_sprints(&[("22 Las vistas", "2026-08-01T00:00:00.000Z")]);

    assert!(muckpile_cli::sprint_add("23_Otra_cosa", &["ACC-1".into()], &provider, &config(), approved).is_err());
    assert!(provider.sprint_additions().is_empty());
}

#[test]
fn adding_nothing_or_an_invalid_id_is_refused_before_asking_anyone() {
    let provider = FakeProvider::new();
    let asked = || -> anyhow::Result<()> { panic!("pidió la frase antes de mirar los argumentos") };

    assert!(muckpile_cli::sprint_add("6533", &[], &provider, &config(), asked).is_err());
    assert!(muckpile_cli::sprint_add("6533", &["../escape".into()], &provider, &config(), asked).is_err());
    assert!(provider.sprint_additions().is_empty());
}

#[test]
fn items_not_approved_are_not_moved() {
    let provider = FakeProvider::new();

    assert!(muckpile_cli::sprint_add("6533", &["ACC-1".into()], &provider, &config(), refused).is_err());

    assert!(provider.sprint_additions().is_empty());
}
