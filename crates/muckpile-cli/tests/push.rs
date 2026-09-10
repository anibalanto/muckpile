//! `push`, first slice: the compare-and-swap against the provider, and the
//! canonicity gate on the body, applied to title and body for items that
//! already carry a real provider id. Resolving pending `@slug` items is a
//! separate slice — it needs search-or-create against the provider, which
//! doesn't exist yet in `muckpile-provider`; `list_summaries` already skips
//! `@slug` files, so `push` simply never sees them.

use muckpile_cli::{push, PushResult};
use muckpile_provider::fake::FakeProvider;
use std::path::Path;
use std::process::Command;

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

    let outcomes = push(view, &provider).unwrap();

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

    let outcomes = push(view, &provider).unwrap();

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

    let outcomes = push(view, &provider).unwrap();

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

    let outcomes = push(view, &provider).unwrap();

    assert_eq!(outcomes[0].result, PushResult::Written { title: false, body: true, body_refused: None });
    let sent = provider.body_adf_of("ACC-1").unwrap();
    assert!(sent.contains("edited body"), "{sent}");
}

/// A text node carrying `code` and `strong` together is exactly what ADF's
/// schema can hold but GFM can't reproduce on the way back through — the
/// same combination `muckpile_core::body`'s own tests use to prove
/// `prune_marks` fires. Seeded directly as ADF, the way a person's edit in
/// Jira's rich editor could have produced it, never through our own
/// (pruning) `body_to_adf`.
const NON_CANONICAL_ADF: &str = r#"{"version":1,"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"bilinker","marks":[{"type":"code"},{"type":"strong"}]}]}]}"#;

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

    let outcomes = push(view, &provider).unwrap();

    let PushResult::Written { title, body, body_refused } = &outcomes[0].result else {
        panic!("expected Written, got {:?}", outcomes[0].result);
    };
    assert!(*title, "title has nothing to do with canonicity");
    assert!(!*body);
    assert!(body_refused.is_some(), "a diff should explain the refusal");
    assert_eq!(provider.title_of("ACC-1").as_deref(), Some("nueva"));
    assert_eq!(provider.body_adf_of("ACC-1").as_deref(), Some(NON_CANONICAL_ADF), "the body must be untouched");
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

    let outcomes = push(view, &provider).unwrap();

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

    let outcomes = push(view, &provider).unwrap();

    let by_id: std::collections::BTreeMap<_, _> = outcomes.iter().map(|o| (o.id.as_str(), &o.result)).collect();
    assert_eq!(by_id["ACC-1"], &PushResult::Unchanged);
    assert_eq!(by_id["ACC-2"], &PushResult::Written { title: true, body: false, body_refused: None });
}
