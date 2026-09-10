//! `commit_paths` and `head_text`: the plumbing that lets `pull` leave a
//! record of what the provider returned, and lets `push` read that record
//! back later without any state file of its own.

use std::path::Path;
use std::process::Command;

fn git_repo() -> tempfile::TempDir {
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

#[test]
fn commits_a_path_that_actually_changed() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();

    let committed = muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    assert!(committed);
    assert_eq!(head_count(repo), 1);
}

#[test]
fn does_nothing_when_the_path_already_matches_head() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();
    muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    // Nothing changed since — same content, written again.
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();
    let committed = muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    assert!(!committed);
    assert_eq!(head_count(repo), 1, "a second, empty commit must not land");
}

#[test]
fn a_second_change_adds_a_second_commit() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();
    muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Finalizada\n---\n").unwrap();
    let committed = muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    assert!(committed);
    assert_eq!(head_count(repo), 2);
}

#[test]
fn does_not_sweep_in_an_unrelated_dirty_file_elsewhere_in_the_repo() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("other-view")).unwrap();
    std::fs::write(repo.join("other-view/ACC-9.task.md"), "---\ntitle: untouched\nstatus: Open\n---\n").unwrap();
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-qm", "seed"]);

    // Someone else's view has an uncommitted edit sitting around.
    std::fs::write(repo.join("other-view/ACC-9.task.md"), "---\ntitle: mid-edit\nstatus: Open\n---\n").unwrap();

    std::fs::create_dir_all(repo.join("my-view")).unwrap();
    std::fs::write(repo.join("my-view/ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();
    muckpile_core::commit_paths(&repo.join("my-view"), &["ACC-1.task.md"], "pull ACC-1").unwrap();

    let out = Command::new("git").arg("-C").arg(repo).args(["status", "--porcelain", "other-view"]).output().unwrap();
    let status = String::from_utf8(out.stdout).unwrap();
    assert!(!status.is_empty(), "the unrelated edit must still be uncommitted: {status:?}");
}

#[test]
fn head_text_is_none_before_anything_is_committed() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();

    assert_eq!(muckpile_core::head_text(repo, "ACC-1.task.md").unwrap(), None);
}

#[test]
fn head_text_is_none_for_a_path_head_never_carried() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();
    muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    assert_eq!(muckpile_core::head_text(repo, "ACC-2.task.md").unwrap(), None);
}

#[test]
fn head_text_reads_back_the_last_committed_content_not_the_working_copy() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: x\nstatus: Open\n---\n").unwrap();
    muckpile_core::commit_paths(repo, &["ACC-1.task.md"], "pull ACC-1").unwrap();

    // Edited locally, not committed again.
    std::fs::write(repo.join("ACC-1.task.md"), "---\ntitle: edited\nstatus: Open\n---\n").unwrap();

    assert_eq!(muckpile_core::head_text(repo, "ACC-1.task.md").unwrap(), Some("---\ntitle: x\nstatus: Open\n---\n".to_string()));
}
