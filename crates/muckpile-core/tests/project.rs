//! Where a project root is, what a given directory counts as inside it, and
//! reading `muckpile.toml`. No provider involved — this is all local.

use muckpile_core::project::{classify, find_project_root, load_project_config, require_root, ItemType, Position};
use std::path::Path;

/// The minimal shape a project root needs for `classify`/`require_root` to
/// have something to classify against.
fn scaffold(root: &Path) {
    std::fs::create_dir_all(root.join(".muckpile")).unwrap();
    std::fs::create_dir_all(root.join("base")).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint/22_Las_vistas")).unwrap();
    std::fs::create_dir_all(root.join("to-work/ACC-1")).unwrap();
}

#[test]
fn finds_the_root_from_deep_inside_a_view() {
    let dir = tempfile::tempdir().unwrap();
    scaffold(dir.path());
    let start = dir.path().join("to-work/ACC-1");
    assert_eq!(find_project_root(&start).unwrap(), dir.path());
}

#[test]
fn finds_nothing_outside_any_project() {
    let dir = tempfile::tempdir().unwrap();
    assert!(find_project_root(dir.path()).is_none());
}

#[test]
fn classifies_every_reserved_position() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    assert_eq!(classify(root, root), Position::Root);
    assert_eq!(classify(root, &root.join("base")), Position::Base);
    assert_eq!(classify(root, &root.join("base/sge")), Position::Base, "a repo under base/ is still base/");
    assert_eq!(classify(root, &root.join("backlog")), Position::Backlog);
    assert_eq!(classify(root, &root.join("backlog/sprint")), Position::BacklogSprint);
    assert_eq!(classify(root, &root.join("backlog/sprint/22_Las_vistas")), Position::SprintView);
    assert_eq!(classify(root, &root.join("to-work")), Position::ToWorkRoot);
    assert_eq!(classify(root, &root.join("to-work/ACC-1")), Position::WorkView);
    assert_eq!(classify(root, &root.join("to-work/ACC-1/code-work/sge")), Position::WorkView, "still inside the same view");
    assert_eq!(classify(root, &root.join("somewhere-else")), Position::Elsewhere);
}

#[test]
fn to_work_only_accepts_the_project_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    assert!(require_root(root, root).is_ok());

    let err = require_root(root, &root.join("backlog/sprint/22_Las_vistas")).unwrap_err();
    assert!(err.to_string().contains("backlog/sprint/22_Las_vistas"), "{err}");

    assert!(require_root(root, &root.join("base")).is_err());
    assert!(require_root(root, &root.join("to-work/ACC-1")).is_err());
}

#[test]
fn loads_repos_and_item_types_from_the_project_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("muckpile.toml"),
        r#"
provider = "jira-rest"
jira_base_url = "https://lamansys.atlassian.net"
jira_project_key = "SGE"
jira_board_id = 12
commit_prefix = "jr"

[repos.sge]
remote = "git@gitlab.lamansys.ar:minsal/sge.git"
branch = "master"

[repos.portal-escolar]
remote = "git@gitlab.lamansys.ar:minsal/portal-escolar.git"
branch = "main"

[item_type]
task = "Tarea"
user-story = "Historia"
epic = "Epic"
question = { type = "Tarea", label = "question" }
"#,
    )
    .unwrap();

    let config = load_project_config(dir.path()).unwrap();
    assert_eq!(config.jira_project_key, "SGE");
    assert_eq!(config.jira_board_id, Some(12));
    assert_eq!(config.commit_prefix, "jr");
    assert_eq!(config.repos["sge"].branch, "master");
    assert_eq!(config.repos["portal-escolar"].remote, "git@gitlab.lamansys.ar:minsal/portal-escolar.git");
    assert_eq!(config.item_type["user-story"], ItemType { jira_type: "Historia".into(), label: None });
    assert_eq!(config.item_type["question"], ItemType { jira_type: "Tarea".into(), label: Some("question".into()) });
}

fn config_with_item_types(item_types: &str) -> anyhow::Result<muckpile_core::project::ProjectConfig> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("muckpile.toml"),
        format!("provider = \"jira-rest\"\njira_base_url = \"https://x\"\njira_project_key = \"SGE\"\ncommit_prefix = \"jr\"\n\n[item_type]\n{item_types}"),
    )
    .unwrap();
    load_project_config(dir.path())
}

/// Two muckpile types on the same provider type with nothing to tell them
/// apart: an item of that type couldn't say which one it is.
#[test]
fn refuses_two_types_sharing_a_provider_type_with_no_label_between_them() {
    let err = config_with_item_types("task = \"Tarea\"\nquestion = \"Tarea\"\n").unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("Tarea") && msg.contains("question") && msg.contains("task"), "{msg}");
}

#[test]
fn refuses_two_types_sharing_a_provider_type_and_a_label() {
    let err = config_with_item_types("task = { type = \"Tarea\", label = \"x\" }\nquestion = { type = \"Tarea\", label = \"x\" }\n").unwrap_err();
    assert!(format!("{err:#}").contains("x"), "{err:#}");
}

#[test]
fn a_shared_provider_type_is_told_apart_by_its_label() {
    let config = config_with_item_types("task = \"Tarea\"\nquestion = { type = \"Tarea\", label = \"question\" }\nepic = \"Epic\"\n").unwrap();
    assert_eq!(config.muckpile_type_of("Tarea", &["question".to_string()]), Some("question"));
    assert_eq!(config.muckpile_type_of("Tarea", &["otra".to_string()]), Some("task"), "the one with no label is the default");
    assert_eq!(config.muckpile_type_of("Tarea", &[]), Some("task"));
    assert_eq!(config.muckpile_type_of("Epic", &[]), Some("epic"));
    assert_eq!(config.muckpile_type_of("Historia", &[]), None);
}

/// Every muckpile type on the provider type carries a label, and the item
/// carries none of them: there's no default to fall back to.
#[test]
fn an_item_with_none_of_the_labels_and_no_default_has_no_type() {
    let config = config_with_item_types("question = { type = \"Tarea\", label = \"question\" }\n").unwrap();
    assert_eq!(config.muckpile_type_of("Tarea", &[]), None);
}

#[test]
fn a_project_without_repos_or_item_type_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://lamansys.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n",
    )
    .unwrap();

    let config = load_project_config(dir.path()).unwrap();
    assert!(config.repos.is_empty());
    assert!(config.item_type.is_empty());
    assert_eq!(config.jira_board_id, None);
}

#[test]
fn a_query_s_view_is_one_level_inside_backlog_queries() {
    let root = Path::new("/multitask/acc");
    assert_eq!(classify(root, &root.join("backlog/queries")), Position::BacklogQueries);
    assert_eq!(classify(root, &root.join("backlog/queries/sin-sprint")), Position::QueryView);
    assert_eq!(classify(root, &root.join("backlog/queries/sin-sprint/ACC-1_data")), Position::QueryView);
}

#[test]
fn declared_queries_are_read_by_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n\n[queries]\nsin-sprint = \"project = ACC AND sprint is empty\"\n",
    )
    .unwrap();
    let config = load_project_config(dir.path()).unwrap();
    assert_eq!(config.queries["sin-sprint"], "project = ACC AND sprint is empty");
}
