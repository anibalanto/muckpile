//! `pull`, first slice: no argument, run standing exactly in `to-work/<id>/`
//! — the same view `to-work` just left empty. Fetches that one item and
//! writes `<id>.<type>.md` into it, saying right then whether its body can
//! be edited locally. Sprints, an explicit id argument, `relation.*`, and
//! the `question` data directory are not this slice.

use muckpile_cli::pull;
use muckpile_core::project::load_project_config;
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::{Comment, Provider};
use std::path::Path;
use std::process::Command;

fn scaffold(root: &Path) {
    muckpile_core::ledger::init(root).unwrap();
    std::fs::create_dir_all(root.join("base")).unwrap();
    std::fs::create_dir_all(root.join("backlog/sprint")).unwrap();
    muckpile_core::ledger::open_view(root, "to-work/ACC-355").unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"ACC\"\ncommit_prefix = \"acc\"\n\n[item_type]\ntask = \"Tarea\"\nquestion = { type = \"Tarea\", label = \"question\" }\n",
    )
    .unwrap();
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {:?} failed", args);
    String::from_utf8(out.stdout).unwrap()
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

/// What was pulled is recorded on the view's provider ref — the ref only
/// ever moved by what the provider said — and the view rebased onto it:
/// neither ahead nor behind.
#[test]
fn records_what_was_pulled_on_the_provider_s_ref_and_rebases_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hola"}]}]}"#;
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, Some(adf));

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    let recorded = muckpile_core::ledger::provider_text(&view, "ACC-355.task.md").unwrap();
    assert_eq!(recorded, Some(std::fs::read_to_string(&path).unwrap()));
    assert_eq!(muckpile_core::ledger::provider_text(&view, ".provider/ACC-355.adf.json").unwrap().as_deref(), Some(adf), "the ADF, as the provider returned it");
    assert!(git(&view, &["status", "-sb"]).starts_with("## to-work/ACC-355...provider/to-work/ACC-355\n"), "neither ahead nor behind");
    assert_eq!(git(&view, &["log", "-1", "--format=%an %s", "provider/to-work/ACC-355"]).trim(), "muckpile pull ACC-355");
}

/// Pulling rebases the view: an edit nobody committed would have to be set
/// aside and put back, so pull refuses — commit it or drop it first.
#[test]
fn refuses_a_view_with_uncommitted_edits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-355", "Tarea", "Vistas de trabajo", "En curso", None, None);
    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();
    std::fs::write(view.join("ACC-355.task.md"), "---\ntitle: a medio editar\nstatus: En curso\n---\n").unwrap();

    let err = pull(root, &view, None, &provider, &config).unwrap_err();

    assert!(err.to_string().contains("sin commitear"), "{err}");
}

/// A person's own commit survives a pull: it's replayed on top of what the
/// provider has now.
#[test]
fn a_person_s_commit_is_replayed_on_top_of_what_the_provider_has_now() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    let adf = |t: &str| format!(r#"{{"version":1,"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{t}"}}]}}]}}"#);
    provider.seed_item("ACC-355", "Tarea", "Vistas", "En curso", None, Some(&adf("uno")));
    let view = root.join("to-work/ACC-355");
    pull(root, &view, None, &provider, &config).unwrap();
    std::fs::write(view.join("notas.md"), "mías\n").unwrap();
    git(&view, &["add", "notas.md"]);
    git(&view, &["-c", "user.name=Ana", "-c", "user.email=ana@x", "commit", "-qm", "notas"]);

    provider.seed_item("ACC-355", "Tarea", "Vistas", "Finalizada", None, Some(&adf("uno")));
    pull(root, &view, None, &provider, &config).unwrap();

    assert!(std::fs::read_to_string(view.join("ACC-355.task.md")).unwrap().contains("status: Finalizada"));
    assert_eq!(std::fs::read_to_string(view.join("notas.md")).unwrap(), "mías\n");
    assert!(git(&view, &["status", "-sb"]).contains("[ahead 1]"));
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
    let before = head_count(&view);
    pull(root, &view, None, &provider, &config).unwrap();

    assert_eq!(head_count(&view), before, "nothing changed on the second pull");
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
    let recorded = muckpile_core::ledger::provider_text(&view, "ACC-355_data/thread/42224.md").unwrap();
    assert_eq!(recorded, Some(reply), "the thread is recorded with the item");
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
    std::fs::write(view.join("ACC-355_data/files/borrador-adr.md"), "no decidido").unwrap(); // never added: the view's own

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
    let body = format!(r#"{{"version":1,"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"Depends on "}},{{"type":"inlineCard","attrs":{{"url":"{}"}}}}]}}]}}"#, provider.item_url("ACC-355"));
    provider.seed_item("ACC-100", "Tarea", "y", "Abierta", None, Some(&body));
    pull(root, &view, Some("ACC-100"), &provider, &config).unwrap();
    let before = head_count(&view);

    provider.seed_item("ACC-355", "Epic", "Vistas", "Abierta", None, None);
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    assert_eq!(path, view.join("ACC-355.epic.md"));
    assert!(!view.join("ACC-355.task.md").exists(), "never two files for the same item");
    let other = std::fs::read_to_string(view.join("ACC-100.task.md")).unwrap();
    assert!(other.contains("[ACC-355](ACC-355.epic.md)"), "{other}");
    assert_eq!(head_count(&view), before + 1, "one pull commit");
    let status = std::process::Command::new("git").arg("-C").arg(&view).args(["status", "--porcelain", "--", "."]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "", "nothing in the view left out of the commit");
}

/// A card to another item comes down as a link to its file, whether or not
/// that file is in the view — so pulling it later doesn't change this body.
#[test]
fn a_card_to_another_item_comes_down_as_a_link_to_its_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-100", "Tarea", "La madre", "Abierta", None, None);
    let body = format!(r#"{{"version":1,"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"Cuelga de "}},{{"type":"inlineCard","attrs":{{"url":"{}"}}}}]}}]}}"#, provider.item_url("ACC-100"));
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, Some(&body));

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Cuelga de [ACC-100](ACC-100.task.md)"), "{text}");
}

/// The type of a cited item is the provider's type and labels, through the
/// same table: a card to a Tarea labeled as a question points at a
/// `.question.md`.
#[test]
fn a_card_to_a_question_comes_down_as_a_link_to_a_question_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    scaffold(root);
    let config = load_project_config(root).unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-338", "Tarea", "¿se hereda?", "Abierta", None, None);
    provider.seed_labels("ACC-338", &["question"]);
    let body = format!(r#"{{"version":1,"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"Bloqueada por "}},{{"type":"inlineCard","attrs":{{"url":"{}"}}}}]}}]}}"#, provider.item_url("ACC-338"));
    provider.seed_item("ACC-355", "Tarea", "Vistas", "Abierta", None, Some(&body));

    let view = root.join("to-work/ACC-355");
    let path = pull(root, &view, None, &provider, &config).unwrap().path;

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[ACC-338](ACC-338.question.md)"), "{text}");
}
