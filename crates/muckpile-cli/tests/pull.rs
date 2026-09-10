//! `pull`, first slice: no argument, run standing exactly in `to-work/<id>/`
//! — the same view `to-work` just left empty. Fetches that one item and
//! writes `<id>.<type>.md` into it, saying right then whether its body can
//! be edited locally. Sprints, an explicit id argument, `relation.*`, and
//! the `question` data directory are not this slice.

use muckpile_cli::pull;
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::Comment;
use std::path::Path;
use std::process::Command;

fn scaffold(root: &Path) {
    std::fs::create_dir_all(root.join(".muckpile")).unwrap();
    std::fs::create_dir_all(root.join("base")).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    std::fs::create_dir_all(root.join("to-work/ACC-355")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n\n[item_type]\ntask = \"Tarea\"\nquestion = { type = \"Tarea\", label = \"question\" }\n",
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
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

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
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

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
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

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
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    let text = std::fs::read_to_string(&path).unwrap();
    let (_, body) = text.split_once("---\n").unwrap();
    let (_, body) = body.split_once("---\n").unwrap();
    assert!(body.contains("hola"), "{text}");
}

#[test]
fn a_body_that_comes_back_the_same_through_markdown_comes_down_editable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hola"}]}]}"#;
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, Some(adf));

    let view = root.join("to-work/ACC-355");
    let pulled = pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(pulled.losses, vec![]);
}

/// Said when the body comes down, not when someone already edited it and
/// ran `push`.
#[test]
fn says_when_the_body_it_brought_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    let numbered = r#"{"version":1,"type":"doc","content":[{"type":"table","attrs":{"isNumberColumnEnabled":true},"content":[
        {"type":"tableRow","content":[{"type":"tableHeader","content":[{"type":"paragraph","content":[{"type":"text","text":"field"}]}]}]},
        {"type":"tableRow","content":[{"type":"tableCell","content":[{"type":"paragraph","content":[{"type":"text","text":"status"}]}]}]}]}]}"#;
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, Some(numbered));

    let view = root.join("to-work/ACC-355");
    let pulled = pull(root, &view, None, &provider, &config).unwrap();

    assert!(!pulled.losses.is_empty(), "a numbered table can't be written back through markdown");
    assert!(pulled.path.exists(), "read-only still comes down: it's the editing that isn't safe");
}

#[test]
fn writes_every_link_with_the_phrase_from_its_own_side_and_underscores_for_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);
    provider.seed_links("ACC-355", &[("is blocked by", "ACC-341"), ("blocks", "ACC-229"), ("is blocked by", "ACC-340")]);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with("---\ntitle: Vistas de trabajo\nstatus: En curso\nrelation.blocks: [ACC-229]\nrelation.is_blocked_by: [ACC-340, ACC-341]\n---\n"),
        "{text}"
    );
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
    let path = pull(root, &view, Some("ACC-100"), &provider, &config).unwrap().path;

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

#[test]
fn a_task_labeled_as_a_question_comes_down_as_a_question() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "¿se hereda?", "Abierta", None, None);
    provider.seed_labels("ACC-355", &["question"]);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    assert_eq!(path, view.join("ACC-355.question.md"));
}

#[test]
fn a_task_with_no_label_comes_down_as_a_task() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    assert_eq!(path, view.join("ACC-355.task.md"));
}

fn paragraph(text: &str) -> String {
    format!(r#"{{"version":1,"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{text}"}}]}}]}}"#)
}

fn comment(id: &str, author: &str, parent: Option<&str>, body_adf: String) -> Comment {
    Comment { id: id.into(), author: author.into(), author_id: format!("{author}-id"), created: "2026-09-02T13:06:47.823-0300".into(), parent: parent.map(Into::into), body_adf }
}

#[test]
fn brings_each_comment_into_the_thread_with_the_one_it_replies_to() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);
    provider.seed_comments("ACC-355", vec![comment("42180", "Ana", None, paragraph("una pregunta")), comment("42224", "Beto", Some("42180"), paragraph("la respuesta"))]);

    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();

    let root_msg = std::fs::read_to_string(view.join("ACC-355_data/thread/42180.md")).unwrap();
    assert_eq!(root_msg, "---\nauthor: Ana\nauthor_id: Ana-id\ncreated: 2026-09-02T13:06:47.823-0300\n---\nuna pregunta\n");
    let reply = std::fs::read_to_string(view.join("ACC-355_data/thread/42224.md")).unwrap();
    assert!(reply.contains("\nin-reply-to: 42180\n"), "{reply}");
    let committed = muckpile_core::head_text(&view, "ACC-355_data/thread/42224.md").unwrap();
    assert_eq!(committed, Some(reply), "the thread lands in the same pull commit");
}

/// A comment an AI wrote starts with `ai: <model>`, the model as code; the
/// thread file carries it in its header, not in its body.
#[test]
fn the_model_that_wrote_a_comment_goes_to_the_header() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);
    let body = r#"{"version":1,"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"ai: "},{"type":"text","text":"claude-opus-5","marks":[{"type":"code"}]}]},
        {"type":"paragraph","content":[{"type":"text","text":"lo que dijo"}]}]}"#;
    provider.seed_comments("ACC-355", vec![comment("42300", "Ana", None, body.to_string())]);

    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();

    let msg = std::fs::read_to_string(view.join("ACC-355_data/thread/42300.md")).unwrap();
    assert!(msg.contains("\nai: claude-opus-5\n---\nlo que dijo\n"), "{msg}");
}

#[test]
fn brings_each_attachment_into_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);
    provider.seed_attachment("ACC-355", "44892", "captura.png", b"png bytes");

    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(std::fs::read(view.join("ACC-355_data/files/captura.png")).unwrap(), b"png bytes");
}

/// Two attachments with the same name both carry their id in front, so
/// neither hides the other.
#[test]
fn two_attachments_with_the_same_name_both_carry_their_id() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);
    provider.seed_attachment("ACC-355", "1", "log.txt", b"primero");
    provider.seed_attachment("ACC-355", "2", "log.txt", b"segundo");

    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(std::fs::read(view.join("ACC-355_data/files/1-log.txt")).unwrap(), b"primero");
    assert_eq!(std::fs::read(view.join("ACC-355_data/files/2-log.txt")).unwrap(), b"segundo");
    assert!(!view.join("ACC-355_data/files/log.txt").exists());
}

/// `files/` also holds the drafts nobody uploaded: `pull` never touches a
/// file it didn't bring.
#[test]
fn a_local_draft_in_files_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);
    provider.seed_attachment("ACC-355", "44892", "captura.png", b"png bytes");
    let view = root.join("to-work/ACC-355");
    std::fs::create_dir_all(view.join("ACC-355_data/files")).unwrap();
    std::fs::write(view.join("ACC-355_data/files/borrador-adr.md"), "no decidido").unwrap();

    pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(std::fs::read_to_string(view.join("ACC-355_data/files/borrador-adr.md")).unwrap(), "no decidido");
}

/// The provider changed the item's type: the file is renamed, links to its
/// old name follow, and it's all one pull commit — never two files for the
/// same item.
#[test]
fn a_type_changed_on_the_provider_renames_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let toml = std::fs::read_to_string(root.join("muckpile.toml")).unwrap();
    std::fs::write(root.join("muckpile.toml"), format!("{toml}epic = \"Epic\"\n")).unwrap();
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, None);
    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();
    std::fs::write(view.join("ACC-100.task.md"), "---\ntitle: y\nstatus: Abierta\n---\nDepends on [Vistas](ACC-355.task.md).\n").unwrap();
    muckpile_core::commit_paths(&view, &["ACC-100.task.md"], "pull ACC-100").unwrap();
    let before = head_count(&view);

    provider.seed_item("ACC-355", "Epic", "Vistas", "Abierta", None, None);
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    assert_eq!(path, view.join("ACC-355.epic.md"));
    assert!(!view.join("ACC-355.task.md").exists(), "never two files for the same item");
    let other = std::fs::read_to_string(view.join("ACC-100.task.md")).unwrap();
    assert!(other.contains("[Vistas](ACC-355.epic.md)"), "{other}");
    assert_eq!(head_count(&view), before + 1, "one pull commit");
    let status = std::process::Command::new("git").arg("-C").arg(&view).args(["status", "--porcelain", "--", "."]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "", "nothing in the view left out of the commit");
}
