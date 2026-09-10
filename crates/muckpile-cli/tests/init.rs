//! `init` is the only command that makes a project: its ledger, a
//! `muckpile.toml` to fill in, and the three reserved folders.

use muckpile_cli::init;
use muckpile_core::project::load_project_config;

#[test]
fn makes_the_ledger_the_config_and_the_three_folders() {
    let dir = tempfile::tempdir().unwrap();

    let project = init(dir.path(), "sge").unwrap();

    assert_eq!(project, dir.path().join("sge"));
    for name in ["base", "backlog", "to-work"] {
        assert!(project.join(name).is_dir(), "{name}");
    }
    let bare = std::process::Command::new("git").arg("-C").arg(project.join(".muckpile")).args(["rev-parse", "--is-bare-repository"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&bare.stdout).trim(), "true", "the ledger has no worktree of its own");
    assert!(load_project_config(&project).is_ok(), "the template reads as a config, placeholders and all");
}

/// A code worktree inside a view belongs to another repository: the ledger
/// never sees it.
#[test]
fn the_ledger_never_sees_a_code_worktree_inside_a_view() {
    let dir = tempfile::tempdir().unwrap();
    let project = init(dir.path(), "sge").unwrap();
    let view = muckpile_core::ledger::open_view(&project, "to-work/SGE-1").unwrap();
    std::fs::create_dir_all(view.join("code-work/sge")).unwrap();
    std::fs::write(view.join("code-work/sge/main.rs"), "fn main() {}").unwrap();

    let status = std::process::Command::new("git").arg("-C").arg(&view).args(["status", "--porcelain"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "");
}

#[test]
fn refuses_a_project_that_already_exists() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), "sge").unwrap();

    assert!(init(dir.path(), "sge").is_err());
}

#[test]
fn refuses_a_name_that_is_not_one_folder() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["", "a/b", "..", ".oculto"] {
        assert!(init(dir.path(), name).is_err(), "{name:?}");
    }
}
