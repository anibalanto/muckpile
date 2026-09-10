//! `code-work add`'s git plumbing against real repos in temp directories —
//! a local path stands in for `origin`, since `git clone`/`fetch` work the
//! same way against one.

use muckpile_core::codework::{add_worktree, derive_branch, ensure_cloned};
use std::path::Path;
use std::process::Command;

fn run(dir: &Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

/// A repo with one commit on `principal`, standing in for `origin`.
fn remote_repo(principal: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path(), &["init", "-q", "-b", principal]);
    run(dir.path(), &["config", "user.email", "test@test"]);
    run(dir.path(), &["config", "user.name", "test"]);
    write(dir.path(), "README.md", "hola\n");
    run(dir.path(), &["add", "-A"]);
    run(dir.path(), &["commit", "-q", "-m", "seed"]);
    dir
}

fn current_branch(dir: &Path) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(["branch", "--show-current"]).output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn derive_branch_takes_the_key_s_number_with_the_project_s_prefix() {
    assert_eq!(derive_branch("SGE-9876", "jr").unwrap(), "jr-9876");
}

#[test]
fn derive_branch_refuses_a_key_with_no_dash() {
    assert!(derive_branch("nodash", "jr").is_err());
}

#[test]
fn ensure_cloned_clones_on_the_principal_branch() {
    let remote = remote_repo("main");
    let dest = tempfile::tempdir().unwrap();
    let base = dest.path().join("base/sge");

    ensure_cloned(remote.path().to_str().unwrap(), &base, "main").unwrap();

    assert!(base.join("README.md").exists());
    assert_eq!(current_branch(&base), "main");
}

#[test]
fn ensure_cloned_does_nothing_when_the_clone_already_exists() {
    let remote = remote_repo("main");
    let dest = tempfile::tempdir().unwrap();
    let base = dest.path().join("base/sge");
    ensure_cloned(remote.path().to_str().unwrap(), &base, "main").unwrap();
    write(&base, "local-only.txt", "not upstream");

    ensure_cloned(remote.path().to_str().unwrap(), &base, "main").unwrap();

    assert!(base.join("local-only.txt").exists(), "a second call must not touch an existing clone");
}

#[test]
fn add_worktree_tracks_a_branch_that_already_exists_on_origin() {
    let remote = remote_repo("main");
    run(remote.path(), &["branch", "jr-9876"]);
    let dest = tempfile::tempdir().unwrap();
    let base = dest.path().join("base/sge");
    ensure_cloned(remote.path().to_str().unwrap(), &base, "main").unwrap();
    let worktree = dest.path().join("to-work/SGE-9876/code-work/sge");

    add_worktree(&base, &worktree, "jr-9876", "main").unwrap();

    assert_eq!(current_branch(&worktree), "jr-9876");
}

#[test]
fn add_worktree_branches_fresh_from_the_principal_when_it_does_not_exist_on_origin() {
    let remote = remote_repo("main");
    let dest = tempfile::tempdir().unwrap();
    let base = dest.path().join("base/sge");
    ensure_cloned(remote.path().to_str().unwrap(), &base, "main").unwrap();
    let worktree = dest.path().join("to-work/SGE-9876/code-work/sge");

    add_worktree(&base, &worktree, "jr-9876", "main").unwrap();

    assert_eq!(current_branch(&worktree), "jr-9876");
    assert!(worktree.join("README.md").exists(), "branched from main, so main's content is there");
}

#[test]
fn add_worktree_branches_from_a_different_starting_point_when_asked() {
    let remote = remote_repo("main");
    run(remote.path(), &["checkout", "-q", "-b", "rc-3.2"]);
    write(remote.path(), "hotfix-base.txt", "only on rc-3.2");
    run(remote.path(), &["add", "-A"]);
    run(remote.path(), &["commit", "-q", "-m", "rc base"]);
    let dest = tempfile::tempdir().unwrap();
    let base = dest.path().join("base/sge");
    ensure_cloned(remote.path().to_str().unwrap(), &base, "main").unwrap();
    let worktree = dest.path().join("to-work/SGE-9876/code-work/sge");

    add_worktree(&base, &worktree, "jr-9876", "rc-3.2").unwrap();

    assert!(worktree.join("hotfix-base.txt").exists(), "should branch from rc-3.2, not main");
}
