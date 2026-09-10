//! `push`: resolves any pending `@slug` the view carries (search-or-create
//! against the provider) and then, for every item that already has a real
//! id, the compare-and-swap against the provider and the canonicity gate on
//! the body.

use muckpile_cli::{push, PushResult};
use muckpile_core::project::ProjectConfig;
use muckpile_provider::fake::FakeProvider;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn config() -> ProjectConfig {
    let mut item_type = BTreeMap::new();
    item_type.insert("task".to_string(), "Tarea".to_string());
    ProjectConfig {
        provider: "jira-rest".into(),
        jira_base_url: "https://x.atlassian.net".into(),
        jira_project_key: "ACC".into(),
        jira_board_id: None,
        commit_prefix: "acc".into(),
        repos: BTreeMap::new(),
        item_type,
    }
}

fn git_view() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path(), &["init", "-q"]);
    run(dir.path(), &["config", "user.email", "test@test"]);
    run(dir.path(), &["config", "user.name", "test"]);
    dir
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

/// Writes and commits an item exactly as `pull` would have left it — the
/// baseline `push`'s compare-and-swap reads back as "the last thing I knew".
fn seed_pulled(view: &Path, id: &str, status: &str, title: &str, body: &str) {
    let filename = format!("{id}.task.md");
    let text = format!("---\ntitle: {title}\nstatus: {status}\n---\n{body}");
    std::fs::write(view.join(&filename), &text).unwrap();
    muckpile_core::commit_paths(view, &[filename.as_str()], &format!("pull {id}")).unwrap();
}

fn edit_title(view: &Path, id: &str, status: &str, title: &str, body: &str) {
    let filename = format!("{id}.task.md");
    let text = format!("---\ntitle: {title}\nstatus: {status}\n---\n{body}");
    std::fs::write(view.join(&filename), &text).unwrap();
}

#[test]
fn reports_unchanged_when_the_file_matches_the_last_pull() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    seed_pulled(view, "ACC-1", "Abierta", "x", "");

    let outcomes = push(view, &provider, &config()).unwrap();

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].result, PushResult::Unchanged);
    assert_eq!(head_count(view), 1, "nothing to commit");
}

#[test]
fn reports_never_pulled_when_the_view_has_no_git_record_for_the_item() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, None);
    // Written by hand, never committed — no `pull` ever ran for this file.
    std::fs::write(view.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Abierta\n---\n").unwrap();

    let outcomes = push(view, &provider, &config()).unwrap();

    assert_eq!(outcomes[0].result, PushResult::NeverPulled);
    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("x"), "nothing should have been sent");
}

#[test]
fn sends_the_title_when_it_changed_and_the_provider_still_matches_the_last_pull() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, None);
    seed_pulled(view, "ACC-1", "Abierta", "vieja", "");
    edit_title(view, "ACC-1", "Abierta", "nueva", "");

    let outcomes = push(view, &provider, &config()).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Written { title: true, body: false, body_refused: None });
    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("nueva"));
    assert_eq!(head_count(view), 2, "the applied title should land as a new commit");
}

#[test]
fn sends_the_body_when_it_changed_and_is_canonical() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    let adf = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"original"}]}]}"#;
    provider.seed_item("ACC-1", "Tarea", "x", "Abierta", None, Some(adf));
    seed_pulled(view, "ACC-1", "Abierta", "x", "original\n");
    edit_title(view, "ACC-1", "Abierta", "x", "edited body\n");

    let outcomes = push(view, &provider, &config()).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Written { title: false, body: true, body_refused: None });
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
fn refuses_a_noncanonical_body_but_still_sends_the_title() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, Some(NON_CANONICAL_ADF));

    // Seed the git baseline from what `pull` would actually have rendered —
    // not from a hand-typed guess at the markdown amdc produces.
    let pulled_body = muckpile_core::body::adf_to_body(NON_CANONICAL_ADF).unwrap();
    seed_pulled(view, "ACC-1", "Abierta", "vieja", &pulled_body);
    edit_title(view, "ACC-1", "Abierta", "nueva", "edited by hand, should never reach the provider\n");

    let outcomes = push(view, &provider, &config()).unwrap();

    let PushResult::Written { title, body, body_refused } = &outcomes[0].result else {
        panic!("expected Written, got {:?}", outcomes[0].result);
    };
    assert!(*title, "title has nothing to do with canonicity");
    assert!(!*body);
    let refused = body_refused.as_ref().expect("the refusal should say why");
    assert!(!refused.losses.is_empty());
    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("nueva"));
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
    seed_pulled(view, "ACC-1", "Abierta", "x", &pulled_body);
    edit_title(view, "ACC-1", "Abierta", "x", &format!("{pulled_body}\nOne more line.\n"));

    let outcomes = push(view, &provider, &config()).unwrap();

    let PushResult::Written { body_refused: Some(refused), .. } = &outcomes[0].result else {
        panic!("expected a refused body, got {:?}", outcomes[0].result);
    };
    let removed = |needle: &str| refused.diff.lines().any(|l| l.starts_with('-') && l.contains(needle));
    let added = |needle: &str| refused.diff.lines().any(|l| l.starts_with('+') && l.contains(needle));
    assert!(removed("isNumberColumnEnabled"), "{}", refused.diff);
    assert!(added("One more line."), "{}", refused.diff);
}

#[test]
fn refuses_when_the_provider_changed_since_the_last_pull() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "vieja", "Abierta", None, None);
    seed_pulled(view, "ACC-1", "Abierta", "vieja", "");

    // Someone (or `transition`) moved the item on the provider's side,
    // without anyone running `pull` again in this view.
    provider.seed_item("ACC-1", "Tarea", "vieja", "Finalizada", None, None);
    edit_title(view, "ACC-1", "Abierta", "nueva", "");

    let outcomes = push(view, &provider, &config()).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Stale);
    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("vieja"), "nothing should have been written");
    assert_eq!(head_count(view), 1, "no commit for a refused item");
}

#[test]
fn only_touches_the_items_that_actually_changed() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "a", "Abierta", None, None);
    provider.seed_item("ACC-2", "Tarea", "b", "Abierta", None, None);
    seed_pulled(view, "ACC-1", "Abierta", "a", "");
    seed_pulled(view, "ACC-2", "Abierta", "b", "");
    edit_title(view, "ACC-2", "Abierta", "b nueva", "");

    let outcomes = push(view, &provider, &config()).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert_eq!(by_id["ACC-1"], &PushResult::Unchanged);
    assert_eq!(by_id["ACC-2"], &PushResult::Written { title: true, body: false, body_refused: None });
}

fn write_pending(view: &Path, slug: &str, title: &str, parent: Option<&str>, body: &str) {
    let mut text = format!("---\ntitle: {title}\n");
    if let Some(p) = parent {
        text.push_str(&format!("parent: {p}\n"));
    }
    text.push_str("---\n");
    text.push_str(body);
    std::fs::write(view.join(format!("{slug}.task.md")), text).unwrap();
}

#[test]
fn creates_a_pending_slug_that_matches_nothing_on_the_provider() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "");

    let outcomes = push(view, &provider, &config()).unwrap();

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

    let outcomes = push(view, &provider, &config()).unwrap();

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

    push(view, &provider, &config()).unwrap();

    assert_eq!(provider.body_adf_of("ACC-9").as_deref(), Some(real_adf), "found means read-only");
}

#[test]
fn a_created_item_sends_its_draft_body_once() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.queue_create("ACC-403", "Tareas por hacer");
    write_pending(view, "@algo", "Un borrador", None, "el cuerpo del borrador\n");

    push(view, &provider, &config()).unwrap();

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

    let outcomes = push(view, &provider, &config()).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert_eq!(by_id["@padre"], &PushResult::Resolved { id: "ACC-100".into(), created: true });
    assert_eq!(by_id["@hijo"], &PushResult::Resolved { id: "ACC-101".into(), created: true });
    let committed = std::fs::read_to_string(view.join("ACC-101.task.md")).unwrap();
    assert!(committed.contains("parent: ACC-100"), "{committed}");
}

#[test]
fn a_slug_with_no_configured_item_type_fails_without_touching_the_rest() {
    let dir = git_view();
    let view = dir.path();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-1", "Tarea", "a", "Abierta", None, None);
    seed_pulled(view, "ACC-1", "Abierta", "a", "");
    std::fs::write(view.join("@raro.user-story.md"), "---\ntitle: sin tipo configurado\n---\n").unwrap();

    let outcomes = push(view, &provider, &config()).unwrap();

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

    let outcomes = push(view, &provider, &config()).unwrap();

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

    let outcomes = push(view, &provider, &config()).unwrap();

    assert_eq!(outcomes.len(), 1, "one row for the resolve, none from the same-run item scan");
}
