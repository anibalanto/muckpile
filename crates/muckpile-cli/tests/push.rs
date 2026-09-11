//! `push`: resolves any pending `@slug` the view carries (search-or-create
//! against the provider) and then, for every item that already has a real
//! id, the compare-and-swap against the provider, the header that only
//! changes by command, and the canonicity gate on the body.

use muckpile_cli::{push, HeaderEdit, PushOutcome, PushResult};
use muckpile_core::project::{ItemType, ProjectConfig};
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::Provider;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// What `push` is about to write, approved.
fn approved_push(_: &[muckpile_cli::Planned]) -> anyhow::Result<()> {
    Ok(())
}

fn config() -> ProjectConfig {
    let mut item_type = BTreeMap::new();
    item_type.insert("task".to_string(), "Tarea".into());
    item_type.insert("question".to_string(), ItemType { jira_type: "Tarea".into(), label: Some("question".into()) });
    ProjectConfig {
        provider: "jira-rest".into(),
        jira_base_url: "https://x.atlassian.net".into(),
        jira_project_key: "ACC".into(),
        jira_board_id: None,
        commit_prefix: "acc".into(),
        repos: BTreeMap::new(),
        item_type,
        queries: BTreeMap::new(),
        auto_update: false,
        auto_comment: false,
    }
}

/// A project with a ledger, and one view in it, `to-work/ACC-1` — what
/// every test here pushes from. Derefs to the view's path.
struct Fixture {
    _dir: tempfile::TempDir,
    view: std::path::PathBuf,
}

impl Fixture {
    fn path(&self) -> &Path {
        &self.view
    }
}

fn git_view() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("acc");
    std::fs::create_dir_all(&root).unwrap();
    muckpile_core::ledger::init(&root).unwrap();
    let view = muckpile_core::ledger::open_view(&root, "to-work/ACC-1").unwrap();
    Fixture { _dir: dir, view }
}

fn run(repo: &Path, args: &[&str]) {
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

/// Pulls `id` into the view from what `provider` holds — the record `push`
/// compares against as "the last thing the provider said".
fn seed_pulled(view: &Path, provider: &FakeProvider, id: &str) {
    let root = view.parent().unwrap().parent().unwrap();
    muckpile_cli::pull(root, view, Some(id), provider, &config()).unwrap();
}

/// A person edits the item and commits the edit — `push` only sends what's
/// committed.
fn edit_file(view: &Path, id: &str, status: &str, title: &str, body: &str) {
    let filename = format!("{id}.task.md");
    let text = format!("---\ntitle: {title}\nstatus: {status}\n---\n{body}");
    std::fs::write(view.join(&filename), &text).unwrap();
    run(view, &["add", &filename]);
    run(view, &["-c", "user.name=Ana", "-c", "user.email=ana@x", "commit", "-qm", "edit"]);
}

fn field(name: &str, written: Option<&str>, provider: Option<&str>) -> HeaderEdit {
    HeaderEdit::Field { name: name.into(), written: written.map(Into::into), provider: provider.map(Into::into) }
}

/// The header only changes by command: a title edited by hand clashes with
/// the provider's, and nothing of the item is sent — the file stays as it
/// is, for `git diff` to show.
#[test]
fn a_title_edited_by_hand_clashes_and_nothing_is_sent() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, None);
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "nueva", "");
    let edited = std::fs::read_to_string(view.join("ACC-1.task.md")).unwrap();
    let before = head_count(view);

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::HeaderClash(vec![field("title", Some("nueva"), Some("vieja"))]));
    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("vieja"));
    assert_eq!(head_count(view), before, "nothing committed");
    assert_eq!(std::fs::read_to_string(view.join("ACC-1.task.md")).unwrap(), edited, "the file is left for git diff");
}

#[test]
fn a_status_edited_by_hand_clashes() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "En curso", None, None);
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Finalizada", "x", "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::HeaderClash(vec![field("status", Some("Finalizada"), Some("En curso"))]));
    assert_eq!(provider.status_of("ACC-1").as_deref(), Some("En curso"));
}

/// A body edit next to a header edit doesn't go either: what records the
/// send is rebuilt from the provider, and would take the header edit with it.
#[test]
fn a_body_edit_next_to_a_header_edit_is_not_sent_either() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(adf));
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "otro", "edited body\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert!(matches!(outcomes[0].result, PushResult::HeaderClash(_)), "{:?}", outcomes[0].result);
    assert_eq!(provider.body_adf_of("ACC-1").as_deref(), Some(adf));
}

/// Each relation added or taken out by hand is its own edit — what `link`
/// or `unlink` would do instead.
#[test]
fn a_relation_edited_by_hand_clashes_one_edit_per_link() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    provider.seed_links("ACC-1", &[("blocks", "ACC-2")]);
    seed_pulled(view, &provider, "ACC-1");
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Abierta\nrelation.blocks: [ACC-3]\n---\n").unwrap();
    run(view, &["-c", "user.name=Ana", "-c", "user.email=ana@x", "commit", "-qam", "edit"]);

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(
        outcomes[0].result,
        PushResult::HeaderClash(vec![
            HeaderEdit::Relation { phrase: "blocks".into(), other: "ACC-2".into(), added: false },
            HeaderEdit::Relation { phrase: "blocks".into(), other: "ACC-3".into(), added: true },
        ])
    );
    assert!(provider.links_created().is_empty());
}

#[test]
fn sends_the_body_when_it_changed_and_is_canonical() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(adf));
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "x", "edited body\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Written { body: true, body_refused: None });
    let sent = provider.body_adf_of("ACC-1").unwrap();
    assert!(sent.contains("edited body"), "{sent}");
}

/// A table with its rows numbered on the provider's side. Its markdown is
/// exactly an unnumbered table's, and that markdown goes to ADF and back to
/// the same markdown — so judged on the markdown it looks canonical, and
/// writing it back would drop the numbering.
const NON_CANONICAL_ADF: &str = r#"{"version":1,"type":"doc","content":[{"type":"table","attrs":{"isNumberColumnEnabled":true},"content":[
    {"type":"tableRow","content":[{"type":"tableHeader","content":[{"type":"paragraph","content":[{"type":"text","text":"field"}]}]}]},
    {"type":"tableRow","content":[{"type":"tableCell","content":[{"type":"paragraph","content":[{"type":"text","text":"status"}]}]}]}]}]}"#;

#[test]
fn refuses_a_noncanonical_body() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, Some(NON_CANONICAL_ADF));

    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "vieja", "edited by hand, should never reach the provider\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let PushResult::Written { body, body_refused } = &outcomes[0].result else {
        panic!("expected Written, got {:?}", outcomes[0].result);
    };
    assert!(!*body);
    let refused = body_refused.as_ref().expect("the refusal should say why");
    assert!(!refused.losses.is_empty());
    assert_eq!(provider.body_adf_of("ACC-1").as_deref(), Some(NON_CANONICAL_ADF), "the body must be untouched");
}

/// The diff is against what the provider holds, so it shows what sending
/// the draft would take away from it — not the draft against its own trip
/// through markdown, which can't know about the numbering at all.
#[test]
fn the_diff_of_a_refused_body_is_against_the_provider_s_adf() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(NON_CANONICAL_ADF));
    let pulled_body = muckpile_core::body::adf_to_body(NON_CANONICAL_ADF).unwrap();
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "x", &format!("{pulled_body}\nOne more line.\n"));

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let PushResult::Written { body_refused: Some(refused), .. } = &outcomes[0].result else {
        panic!("expected a refused body, got {:?}", outcomes[0].result);
    };
    let removed = |needle: &str| refused.diff.lines().any(|l| l.starts_with('-') && l.contains(needle));
    let added = |needle: &str| refused.diff.lines().any(|l| l.starts_with('+') && l.contains(needle));
    assert!(removed("isNumberColumnEnabled"), "{}", refused.diff);
    assert!(added("One more line."), "{}", refused.diff);
}

/// The provider moved since the view last heard from it: nothing is
/// written. What it has now is recorded on its ref, and the view rebased
/// onto it — the person's edit on top — to look at before pushing again.
#[test]
fn refuses_when_the_provider_changed_since_the_last_pull() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, Some(adf));
    seed_pulled(view, &provider, "ACC-1");

    // Someone (or `transition`) moved the item on the provider's side,
    // without anyone running `pull` again in this view.
    provider.seed_item("ACC-1", "Tarea", "vieja", "Finalizada", None, Some(adf));
    edit_file(view, "ACC-1", "Abierta", "vieja", "original\n\nmás cuerpo\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Stale);
    assert_eq!(provider.body_adf_of("ACC-1").as_deref(), Some(adf), "nothing should have been written");
    let text = std::fs::read_to_string(view.join("ACC-1.task.md")).unwrap();
    assert!(text.contains("status: Finalizada") && text.contains("más cuerpo"), "recorded, and the edit rebased on top: {text}");
}

/// When what the provider did and what the person edited touch the same
/// lines, the rebase stops, for the person to settle with git — push says
/// so instead of guessing.
#[test]
fn a_clash_with_what_the_provider_did_stops_the_rebase_for_a_person() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, None);
    seed_pulled(view, &provider, "ACC-1");
    provider.seed_item("ACC-1", "Tarea", "vieja", "Finalizada", None, None);
    edit_file(view, "ACC-1", "En curso", "vieja", "");

    let err = push(view, &provider, &config(), approved_push).unwrap_err();

    assert!(err.to_string().contains("rebase"), "{err}");
    assert!(muckpile_core::ledger::rebasing(view).unwrap());
}

/// `push` sends what's committed: an edit nobody committed is refused, not
/// guessed at.
#[test]
fn refuses_a_view_with_uncommitted_edits() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    seed_pulled(view, &provider, "ACC-1");
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Abierta\n---\nsin commitear\n").unwrap();

    let err = push(view, &provider, &config(), approved_push).unwrap_err();

    assert!(err.to_string().contains("sin commitear"), "{err}");
}

/// After a push the view is up to date: the edit's own commit went away,
/// because the provider's ref now holds the same change.
#[test]
fn after_a_push_the_view_is_neither_ahead_nor_behind() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(adf));
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "x", "editado\n");

    push(view, &provider, &config(), approved_push).unwrap();

    let status = Command::new("git").arg("-C").arg(view).args(["status", "-sb"]).output().unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).starts_with("## to-work/ACC-1...provider/to-work/ACC-1\n"), "{}", String::from_utf8_lossy(&status.stdout));
}

/// A draft's body goes out in the form it comes back in: when that differs
/// from what was written — a bold run cut by a code span — the draft is
/// rewritten to it first, in a commit of its own, signed as the tool.
#[test]
fn a_draft_is_rewritten_to_its_canonical_form_in_its_own_commit_before_it_goes() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "**el `reach`, y el que falla**\n");

    push(view, &provider, &config(), approved_push).unwrap();

    let log = Command::new("git").arg("-C").arg(view).args(["log", "--format=%an %s"]).output().unwrap();
    let log = String::from_utf8_lossy(&log.stdout);
    assert!(log.contains("muckpile forma canónica de @algo.task"), "{log}");
    let text = std::fs::read_to_string(view.join("ACC-403.task.md")).unwrap();
    assert!(text.contains("**el** `reach`**, y el que falla**"), "{text}");
}

/// The order that keeps the rebase from clashing: the provider's answer is
/// recorded, the view rebased onto it, and only then the draft retired.
#[test]
fn resolving_records_new_on_the_provider_s_ref_before_the_rename() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "");

    push(view, &provider, &config(), approved_push).unwrap();

    let provider_log = Command::new("git").arg("-C").arg(view).args(["log", "-1", "--format=%s", "provider/to-work/ACC-1"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&provider_log.stdout).trim(), "new @algo");
    let view_log = Command::new("git").arg("-C").arg(view).args(["log", "-1", "--format=%s"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&view_log.stdout).trim(), "rename @algo -> ACC-403");
    assert!(!view.join("@algo.task.md").exists());
}

#[test]
fn only_touches_the_items_that_actually_changed() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "a", "Abierta", None, None);
    provider.seed_item("ACC-2", "Tarea", "b", "Abierta", None, None);
    seed_pulled(view, &provider, "ACC-1");
    seed_pulled(view, &provider, "ACC-2");
    edit_file(view, "ACC-2", "Abierta", "b", "a body edit\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert_eq!(by_id["ACC-1"], &PushResult::Unchanged);
    assert_eq!(by_id["ACC-2"], &PushResult::Written { body: true, body_refused: None });
}

/// A draft written and left uncommitted — what `push` leaves local.
fn write_local(view: &Path, slug: &str, title: &str, parent: Option<&str>, body: &str) {
    let mut text = format!("---\ntitle: {title}\n");
    if let Some(p) = parent {
        text.push_str(&format!("parent: {p}\n"));
    }
    text.push_str("---\n");
    text.push_str(body);
    std::fs::write(view.join(format!("{slug}.task.md")), text).unwrap();
}

/// A draft a person wrote and committed — what `push` sends.
fn write_pending(view: &Path, slug: &str, title: &str, parent: Option<&str>, body: &str) {
    write_local(view, slug, title, parent, body);
    commit_all(view);
}

/// A person commits everything the view has, new files included.
fn commit_all(view: &Path) {
    run(view, &["add", "-A"]);
    run(view, &["-c", "user.name=Ana", "-c", "user.email=ana@x", "commit", "-qm", "borrador"]);
}

#[test]
fn creates_a_pending_slug_that_matches_nothing_on_the_provider() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].id, "@algo");
    assert_eq!(outcomes[0].result, PushResult::Resolved { id: "ACC-403".into(), created: true });
    assert!(!view.join("@algo.task.md").exists());
    assert!(view.join("ACC-403.task.md").exists());
    assert_eq!(provider.title_of("ACC-403").as_deref(), Some("Un borrador"));
}

#[test]
fn finds_a_pending_slug_that_already_exists_on_the_provider_instead_of_creating_it() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-9", "Tarea", "Ya existe", "Finalizada", None, None);
    // No queued key: `create_item` would panic on an empty queue, so this
    // only passes if `push` really finds it instead of creating it.
    write_pending(view, "@algo", "Ya existe", None, "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Resolved { id: "ACC-9".into(), created: false });
    let committed = std::fs::read_to_string(view.join("ACC-9.task.md")).unwrap();
    assert!(committed.contains("status: Finalizada"), "{committed}");
}

#[test]
fn a_found_item_never_gets_its_body_overwritten() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    let real_adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"lo real"}]}]}"#;
    provider.seed_item("ACC-9", "Tarea", "Ya existe", "Abierta", None, Some(real_adf));
    write_pending(view, "@algo", "Ya existe", None, "un borrador que nadie pidió subir\n");

    push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(provider.body_adf_of("ACC-9").as_deref(), Some(real_adf), "found means read-only");
}

#[test]
fn a_created_item_sends_its_draft_body_once() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "el cuerpo del borrador\n");

    push(view, &provider, &config(), approved_push).unwrap();

    let sent = provider.body_adf_of("ACC-403").unwrap();
    assert!(sent.contains("el cuerpo del borrador"), "{sent}");
}

#[test]
fn translates_an_in_batch_parent_to_the_freshly_resolved_id() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-100", "Tareas por hacer");
    provider.queue_create("ACC-101", "Tareas por hacer");
    write_pending(view, "@padre", "La épica", None, "");
    write_pending(view, "@hijo", "La tarea", Some("@padre"), "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert_eq!(by_id["@padre"], &PushResult::Resolved { id: "ACC-100".into(), created: true });
    assert_eq!(by_id["@hijo"], &PushResult::Resolved { id: "ACC-101".into(), created: true });
    let committed = std::fs::read_to_string(view.join("ACC-101.task.md")).unwrap();
    assert!(committed.contains("parent: ACC-100"), "{committed}");
}

#[test]
fn a_draft_new_wrote_with_a_parent_is_created_under_it() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-339", "Épica", "La épica", "En curso", None, None);
    provider.queue_create("ACC-403", "Tareas por hacer");
    muckpile_cli::new(view, "task", "La tarea", Some("ACC-339"), None).unwrap();
    commit_all(view);

    push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(provider.parent_of("ACC-403").as_deref(), Some("ACC-339"));
    let committed = std::fs::read_to_string(view.join("ACC-403.task.md")).unwrap();
    assert!(committed.contains("parent: ACC-339\n"), "{committed}");
}

#[test]
fn a_slug_with_no_configured_item_type_fails_without_touching_the_rest() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "a", "Abierta", None, None);
    seed_pulled(view, &provider, "ACC-1");
    std::fs::write(view.join("@raro.user-story.md"), "---\ntitle: sin tipo configurado\n---\n").unwrap();
    commit_all(view);

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert!(matches!(by_id["@raro"], PushResult::ResolveFailed(_)));
    assert_eq!(by_id["ACC-1"], &PushResult::Unchanged, "an unrelated item keeps going");
    assert!(view.join("@raro.user-story.md").exists(), "left alone, still pending");
}

#[test]
fn a_dependency_that_fails_to_resolve_blocks_only_what_depends_on_it() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    std::fs::write(view.join("@padre.user-story.md"), "---\ntitle: sin tipo configurado\n---\n").unwrap();
    write_pending(view, "@hijo", "depende del padre", Some("@padre"), "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert!(matches!(by_id["@padre"], PushResult::ResolveFailed(_)));
    assert!(matches!(by_id["@hijo"], PushResult::ResolveFailed(_)), "{:?}", by_id["@hijo"]);
    assert!(view.join("@hijo.task.md").exists(), "never renamed: its parent never got a key");
}

#[test]
fn a_resolved_item_is_not_reported_again_as_unchanged_in_the_same_run() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes.len(), 1, "one row for the resolve, none from the same-run item scan");
}

fn write_pending_with(view: &Path, slug: &str, title: &str, header: &str) {
    std::fs::write(view.join(format!("{slug}.task.md")), format!("---\ntitle: {title}\n{header}---\n")).unwrap();
    commit_all(view);
}

#[test]
fn a_created_draft_carries_its_relations_to_the_provider() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    provider.seed_item("ACC-229", "Tarea", "Lo bloqueado", "Abierta", None, None);
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending_with(view, "@pregunta", "¿se hereda?", "relation.blocks: ACC-229\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes, vec![PushOutcome { id: "@pregunta".into(), result: PushResult::Resolved { id: "ACC-403".into(), created: true } }]);
    assert_eq!(provider.links_created(), vec![("Blocks".to_string(), "ACC-403".to_string(), "ACC-229".to_string())]);
    let committed = std::fs::read_to_string(view.join("ACC-403.task.md")).unwrap();
    assert!(committed.contains("relation.blocks: [ACC-229]\n"), "the header rebuilt from the provider keeps it: {committed}");
}

/// The key is the provider's phrase with `_` for each space.
#[test]
fn a_relation_key_with_underscores_is_the_provider_s_phrase() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    provider.seed_item("ACC-229", "Tarea", "Lo que bloquea", "Abierta", None, None);
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending_with(view, "@algo", "bloqueada", "relation.is_blocked_by: [ACC-229]\n");

    push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(provider.links_created(), vec![("Blocks".to_string(), "ACC-229".to_string(), "ACC-403".to_string())]);
}

#[test]
fn a_relation_to_another_draft_in_the_batch_links_to_its_resolved_id() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    provider.queue_create("ACC-100", "Tareas por hacer");
    provider.queue_create("ACC-101", "Tareas por hacer");
    write_pending_with(view, "@pregunta", "la pregunta", "relation.blocks: [@tarea]\n");
    write_pending_with(view, "@tarea", "la tarea", "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let resolved: std::collections::BTreeMap<_, _> = outcomes
        .iter()
        .filter_map(|o| match &o.result {
            PushResult::Resolved { id, .. } => Some((o.id.as_str(), id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(provider.links_created(), vec![("Blocks".to_string(), resolved["@pregunta"].clone(), resolved["@tarea"].clone())]);
}

/// The item is created all the same; what failed is said, with the command
/// that retries it — the header rebuilt from the provider won't carry it.
#[test]
fn a_relation_the_provider_has_no_phrase_for_is_reported_with_the_command_to_retry() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending_with(view, "@algo", "un borrador", "relation.depends: ACC-229\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Resolved { id: "ACC-403".into(), created: true });
    let PushResult::RelationFailed { phrase, other, .. } = &outcomes[1].result else {
        panic!("expected the failed relation to be reported, got {outcomes:?}");
    };
    assert_eq!((outcomes[1].id.as_str(), phrase.as_str(), other.as_str()), ("ACC-403", "depends", "ACC-229"));
    assert!(provider.links_created().is_empty());
}

/// Only a created item gets what its draft declares — the same as its body
/// and its parent.
#[test]
fn a_found_item_gets_no_relations_from_the_draft() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_link_types(&[("Blocks", "blocks", "is blocked by")]);
    provider.seed_item("ACC-9", "Tarea", "Ya existe", "Abierta", None, None);
    write_pending_with(view, "@algo", "Ya existe", "relation.blocks: ACC-229\n");

    push(view, &provider, &config(), approved_push).unwrap();

    assert!(provider.links_created().is_empty());
}

/// A question created on a board that has no question type of its own goes
/// up as that type, labeled — the label is what brings it back as a question.
#[test]
fn a_created_question_carries_its_label() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    std::fs::write(view.join("@se-hereda.question.md"), "---\ntitle: ¿se hereda?\n---\n").unwrap();
    commit_all(view);

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Resolved { id: "ACC-403".into(), created: true });
    assert_eq!(provider.labels_of("ACC-403"), vec!["question".to_string()]);
    assert!(view.join("ACC-403.question.md").exists(), "it comes back as a question");
}

/// The search before creating narrows by the label too: a plain task with
/// the same title isn't the question this draft is.
#[test]
fn a_pending_question_is_not_found_as_a_task_with_the_same_title() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-9", "Tarea", "¿se hereda?", "Abierta", None, None);
    provider.queue_create("ACC-403", "Tareas por hacer");
    std::fs::write(view.join("@se-hereda.question.md"), "---\ntitle: ¿se hereda?\n---\n").unwrap();
    commit_all(view);

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Resolved { id: "ACC-403".into(), created: true });
}

/// What goes up is a card; the body that comes back reads the same, so the
/// next push doesn't see a change nobody made.
#[test]
fn a_link_to_an_item_file_goes_up_as_a_card() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-100", "Tarea", "La madre", "Abierta", None, None);
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(adf));
    seed_pulled(view, &provider, "ACC-1");
    edit_file(view, "ACC-1", "Abierta", "x", "Cuelga de [ACC-100](ACC-100.task.md).\n");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Written { body: true, body_refused: None });
    let sent = provider.body_adf_of("ACC-1").unwrap();
    assert!(sent.contains(r#""type":"inlineCard""#) && sent.contains(&provider.item_url("ACC-100")), "{sent}");
    let again = push(view, &provider, &config(), approved_push).unwrap();
    assert_eq!(again[0].result, PushResult::Unchanged, "{again:?}");
}

/// `push` sends what's committed: a draft nobody committed isn't created,
/// and `push` says it saw it and that it stays in the view.
#[test]
fn a_draft_nobody_committed_stays_local() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    // No queued key: `create_item` would panic on an empty queue.
    write_local(view, "@seguimiento", "Una tarea de seguimiento", None, "");
    let before = head_count(view);

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    assert_eq!(outcomes, vec![PushOutcome { id: "@seguimiento".into(), result: PushResult::Local }]);
    assert!(view.join("@seguimiento.task.md").exists());
    assert_eq!(head_count(view), before, "nobody committed it, and neither does push");
    let status = Command::new("git").arg("-C").arg(view).args(["status", "--porcelain"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "?? @seguimiento.task.md\n");
}

/// A committed draft that hangs from one nobody committed has no id to
/// give its parent, so it waits for it — and says so.
#[test]
fn a_draft_that_names_a_local_one_waits_for_it() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    write_pending(view, "@hijo", "La tarea", Some("@padre"), "");
    write_local(view, "@padre", "La épica", None, "");

    let outcomes = push(view, &provider, &config(), approved_push).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert_eq!(by_id["@padre"], &PushResult::Local);
    let PushResult::ResolveFailed(reason) = by_id["@hijo"] else {
        panic!("expected @hijo to wait for @padre, got {:?}", by_id["@hijo"]);
    };
    assert!(reason.contains("@padre") && reason.contains("local"), "{reason}");
    assert!(view.join("@hijo.task.md").exists());
}

/// The person's commit is the draft's: `push` commits nothing in its place,
/// only its own commits on top — the rename.
#[test]
fn a_committed_draft_keeps_the_person_s_commit() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "");

    push(view, &provider, &config(), approved_push).unwrap();

    let log = Command::new("git").arg("-C").arg(view).args(["log", "--format=%an %s"]).output().unwrap();
    let log = String::from_utf8_lossy(&log.stdout);
    assert!(log.contains("Ana borrador"), "{log}");
    assert!(!log.contains("muckpile borrador"), "{log}");
}
