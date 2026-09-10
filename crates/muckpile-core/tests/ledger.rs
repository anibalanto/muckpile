//! The ledger: a project's own git, `.muckpile/`, with one worktree per view
//! and two refs per view — the view's branch, and the provider's ref it sits
//! on. Real git, in a temp directory; no provider.

use muckpile_core::ledger::{self, Rebase};
use std::path::Path;
use std::process::Command;

fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// A person's own commit in a view, with their own identity.
fn person_commits(view: &Path, file: &str, text: &str) {
    std::fs::write(view.join(file), text).unwrap();
    git_out(view, &["add", file]);
    git_out(view, &["-c", "user.name=Ana", "-c", "user.email=ana@x", "commit", "-qm", "edit"]);
}

fn project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("acc");
    std::fs::create_dir_all(&project).unwrap();
    ledger::init(&project).unwrap();
    (dir, project)
}

#[test]
fn init_makes_the_ledger_a_git_with_no_worktree_of_its_own() {
    let (_dir, project) = project();
    assert_eq!(git_out(&project.join(".muckpile"), &["rev-parse", "--is-bare-repository"]).trim(), "true");
}

/// A view is a worktree on its own branch, with the provider's ref as its
/// upstream — so `git status` alone says how far ahead and behind it is.
#[test]
fn a_view_is_a_worktree_on_its_branch_with_the_provider_s_ref_upstream() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();

    assert_eq!(view, project.join("to-work/ACC-1"));
    assert_eq!(git_out(&view, &["symbolic-ref", "--short", "HEAD"]).trim(), "to-work/ACC-1");
    assert!(git_out(&view, &["status", "-sb"]).starts_with("## to-work/ACC-1...provider/to-work/ACC-1"));
}

/// Recording never touches the view: it's a commit on the provider's ref,
/// and the view is behind until it rebases.
#[test]
fn recording_commits_on_the_provider_s_ref_and_leaves_the_view_behind() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();

    let recorded = ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"---\ntitle: x\n---\n".to_vec()))], "pull ACC-1").unwrap();

    assert!(recorded);
    assert!(!view.join("ACC-1.task.md").exists(), "the view didn't move");
    assert!(git_out(&view, &["status", "-sb"]).contains("[behind 1]"));
    assert_eq!(ledger::provider_file(&view, "ACC-1.task.md").unwrap().as_deref(), Some(&b"---\ntitle: x\n---\n"[..]));
}

#[test]
fn recording_what_is_already_there_makes_no_commit() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    let change = [("ACC-1.task.md".to_string(), Some(b"x".to_vec()))];
    ledger::record(&view, &change, "pull ACC-1").unwrap();

    assert!(!ledger::record(&view, &change, "pull ACC-1").unwrap());
}

#[test]
fn a_file_the_provider_no_longer_has_is_recorded_as_gone() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"x".to_vec()))], "pull ACC-1").unwrap();

    ledger::record(&view, &[("ACC-1.task.md".into(), None)], "pull ACC-1").unwrap();

    assert_eq!(ledger::provider_file(&view, "ACC-1.task.md").unwrap(), None);
}

/// What the tool records it signs as itself; what a person commits keeps
/// the person's name.
#[test]
fn the_tool_signs_its_commits_and_a_person_keeps_theirs() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"x\n".to_vec()))], "pull ACC-1").unwrap();
    assert_eq!(ledger::rebase(&view).unwrap(), Rebase::Done);
    person_commits(&view, "notes.md", "mine\n");

    assert_eq!(git_out(&view, &["log", "-1", "--format=%an", "provider/to-work/ACC-1"]).trim(), "muckpile");
    assert_eq!(git_out(&view, &["log", "-1", "--format=%an"]).trim(), "Ana");
}

/// The view's own commits are replayed on top of what the provider has now.
#[test]
fn rebasing_puts_the_view_s_own_commits_on_top_of_the_provider() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"a\nb\n".to_vec()))], "pull ACC-1").unwrap();
    ledger::rebase(&view).unwrap();
    person_commits(&view, "ACC-1.task.md", "a\nb edited\n");
    ledger::record(&view, &[("ACC-2.task.md".into(), Some(b"other\n".to_vec()))], "pull ACC-2").unwrap();

    assert_eq!(ledger::rebase(&view).unwrap(), Rebase::Done);

    assert_eq!(std::fs::read_to_string(view.join("ACC-1.task.md")).unwrap(), "a\nb edited\n");
    assert!(view.join("ACC-2.task.md").exists());
    assert!(git_out(&view, &["status", "-sb"]).contains("[ahead 1]"));
}

/// An edit that the provider already has — the push that sent it recorded
/// it — goes away on rebase instead of staying as a commit of nothing.
#[test]
fn a_commit_the_provider_already_has_goes_away_on_rebase() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"a\n".to_vec()))], "pull ACC-1").unwrap();
    ledger::rebase(&view).unwrap();
    person_commits(&view, "ACC-1.task.md", "b\n");
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"b\n".to_vec()))], "push ACC-1").unwrap();

    ledger::rebase(&view).unwrap();

    assert!(git_out(&view, &["status", "-sb"]).starts_with("## to-work/ACC-1...provider/to-work/ACC-1\n"), "neither ahead nor behind");
}

#[test]
fn a_clash_stops_the_rebase_for_a_person_to_settle() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"a\n".to_vec()))], "pull ACC-1").unwrap();
    ledger::rebase(&view).unwrap();
    person_commits(&view, "ACC-1.task.md", "mine\n");
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"theirs\n".to_vec()))], "pull ACC-1").unwrap();

    assert!(matches!(ledger::rebase(&view).unwrap(), Rebase::Stopped(_)));
    assert!(ledger::rebasing(&view).unwrap());
}

/// An uncommitted edit to a tracked file counts; a new file nobody added
/// yet — a fresh draft — doesn't.
#[test]
fn tracked_changes_are_edits_to_what_git_tracks() {
    let (_dir, project) = project();
    let view = ledger::open_view(&project, "to-work/ACC-1").unwrap();
    ledger::record(&view, &[("ACC-1.task.md".into(), Some(b"a\n".to_vec()))], "pull ACC-1").unwrap();
    ledger::rebase(&view).unwrap();
    std::fs::write(view.join("@borrador.task.md"), "draft\n").unwrap();
    assert!(ledger::tracked_changes(&view).unwrap().is_empty());

    std::fs::write(view.join("ACC-1.task.md"), "edited\n").unwrap();
    assert_eq!(ledger::tracked_changes(&view).unwrap(), vec!["ACC-1.task.md".to_string()]);
}
