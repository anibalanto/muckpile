//! `code-work add`, run inside a `to-work/<id>/` view: clones `base/<repo>`
//! on demand, and adds `code-work/<repo>/` tracking the branch `commit_prefix`
//! derives, or branching it fresh — `--from`/`--branch` override the
//! starting point and the name respectively.

use muckpile_cli::code_work_add;
use muckpile_core::project::{load_project_config, ProjectConfig};
use std::path::Path;
use std::process::Command;

fn run(dir: &Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

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

/// A project root with `to-work/<id>/` already there and a `sge` repo
/// configured against `remote`.
fn scaffold(root: &Path, id: &str, remote: &Path) -> ProjectConfig {
    muckpile_core::ledger::init(root).unwrap();
    muckpile_core::ledger::open_view(root, &format!("to-work/{id}")).unwrap();
    std::fs::write(
        root.join("muckpile.toml"),
        format!(
            "provider = \"jira-rest\"\njira_base_url = \"https://x.atlassian.net\"\njira_project_key = \"SGE\"\ncommit_prefix = \"jr\"\n\n[repos.sge]\nremote = \"{}\"\nbranch = \"main\"\n",
            remote.display()
        ),
    )
    .unwrap();
    load_project_config(root).unwrap()
}

#[test]
fn adds_a_worktree_tracking_the_derived_branch_when_it_already_exists() {
    let remote = remote_repo("main");
    run(remote.path(), &["branch", "jr-9876"]);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());
    let view = root.join("to-work/SGE-9876");

    let worktree = code_work_add(root, &view, "sge", None, None, &config).unwrap();

    assert_eq!(worktree, view.join("code-work/sge"));
    assert_eq!(current_branch(&worktree), "jr-9876");
}

#[test]
fn clones_base_on_demand_and_branches_fresh_from_the_principal() {
    let remote = remote_repo("main");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());
    let view = root.join("to-work/SGE-9876");

    assert!(!root.join("base/sge").exists());
    let worktree = code_work_add(root, &view, "sge", None, None, &config).unwrap();

    assert!(root.join("base/sge").exists());
    assert_eq!(current_branch(&worktree), "jr-9876");
    assert!(worktree.join("README.md").exists());
}

#[test]
fn from_overrides_the_starting_point_for_a_new_branch() {
    let remote = remote_repo("main");
    run(remote.path(), &["checkout", "-q", "-b", "rc-3.2"]);
    write(remote.path(), "hotfix-base.txt", "x");
    run(remote.path(), &["add", "-A"]);
    run(remote.path(), &["commit", "-q", "-m", "rc base"]);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());
    let view = root.join("to-work/SGE-9876");

    let worktree = code_work_add(root, &view, "sge", Some("rc-3.2"), None, &config).unwrap();

    assert!(worktree.join("hotfix-base.txt").exists());
}

#[test]
fn branch_overrides_the_derived_name_entirely() {
    let remote = remote_repo("main");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());
    let view = root.join("to-work/SGE-9876");

    let worktree = code_work_add(root, &view, "sge", None, Some("split-front"), &config).unwrap();

    assert_eq!(current_branch(&worktree), "split-front");
}

#[test]
fn refuses_a_repo_not_listed_in_muckpile_toml() {
    let remote = remote_repo("main");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());
    let view = root.join("to-work/SGE-9876");

    let err = code_work_add(root, &view, "portal-escolar", None, None, &config).unwrap_err();
    assert!(err.to_string().contains("portal-escolar"), "{err}");
}

#[test]
fn refuses_to_run_outside_a_work_view() {
    let remote = remote_repo("main");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());

    let err = code_work_add(root, root, "sge", None, None, &config).unwrap_err();
    assert!(err.to_string().contains("to-work"), "{err}");
}

#[test]
fn refuses_a_worktree_that_already_exists() {
    let remote = remote_repo("main");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = scaffold(root, "SGE-9876", remote.path());
    let view = root.join("to-work/SGE-9876");
    code_work_add(root, &view, "sge", None, None, &config).unwrap();

    let err = code_work_add(root, &view, "sge", None, None, &config).unwrap_err();
    assert!(err.to_string().contains("code-work/sge"), "{err}");
}
