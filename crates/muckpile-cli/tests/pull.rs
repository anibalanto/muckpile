//! `pull`, first slice: no argument, run standing exactly in `to-work/<id>/`
//! — the same view `to-work` just left empty. Fetches that one item and
//! writes `<id>.<type>.md` into it. Sprints, an explicit id argument,
//! `relation.*`, and the `question` data directory are not this slice.

use muckpile_cli::pull;
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use std::path::Path;
use std::process::Command;

fn scaffold(root: &Path) {
    std::fs::create_dir_all(root.join(".muckpile")).unwrap();
    std::fs::create_dir_all(root.join("base")).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::create_dir_all(root.join("to-work/ACC-355")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n\n[item_type]\ntask = \"Tarea\"\n",
    )
    .unwrap();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "test@test"]);
    git(root, &["config", "user.name", "test"]);
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(repo).args(args).status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn head_count(repo: &Path) -> usize {
    let out = Command::new("git").arg("-C").arg(repo).args(["rev-list", "--count", "HEAD"]).output().unwrap();
    if !out.status.success() {
        return 0;
    }
    String::from_utf8(out.stdout).unwrap().trim().parse().unwrap()
}

#[test]
fn writes_the_view_s_own_item_with_no_argument() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(path, view.join("ACC-355.task.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("---\ntitle: Vistas de trabajo\nstatus: En curso\n---\n"), "{text}");
}

#[test]
fn leaves_a_git_record_of_what_was_pulled() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(head_count(&view), 1, "the fetched item should land as its own commit");
    let committed = muckpile_core::head_text(&view, "ACC-355.task.md").unwrap();
    assert_eq!(committed, Some(std::fs::read_to_string(&path).unwrap()));
}

#[test]
fn pulling_the_same_state_again_does_not_add_an_empty_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);

    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();
    pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(head_count(&view), 1, "nothing changed on the second pull");
}

#[test]
fn carries_the_parent_when_the_item_has_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", Some("ACC-100"), None);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\nparent: ACC-100\n"), "{text}");
}

#[test]
fn converts_the_description_to_a_markdown_body() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hola"}]}]}"#;
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, Some(adf));

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let (_, body) = text.split_once("---\n").unwrap();
    let (_, body) = body.split_once("---\n").unwrap();
    assert!(body.contains("hola"), "{text}");
}

#[test]
fn refuses_outside_a_work_view() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();

    let err = pull(root, root, None, &provider, &config).unwrap_err();
    assert!(err.to_string().contains("to-work"), "{err}");
}

#[test]
fn an_explicit_id_pulls_a_different_item_into_the_same_view() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);
    provider.seed_item("ACC-100", "Tarea", "La épica madre", "Abierta", None, None);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, Some("ACC-100"), &provider, &config).unwrap();

    assert_eq!(path, view.join("ACC-100.task.md"), "lands in the view standing, not a new one");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("title: La épica madre"), "{text}");
    assert!(!view.join("ACC-355.task.md").exists(), "pull didn't also re-fetch the anchor by itself");
}

#[test]
fn refuses_an_explicit_id_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();

    let view = root.join("to-work/ACC-355");
    assert!(pull(root, &view, Some("../escape"), &provider, &config).is_err());
}

#[test]
fn refuses_a_jira_type_with_no_configured_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Historia", "Una historia", "Abierta", None, None);

    let view = root.join("to-work/ACC-355");
    let err = pull(root, &view, None, &provider, &config).unwrap_err();
    assert!(err.to_string().contains("Historia"), "{err}");
}
