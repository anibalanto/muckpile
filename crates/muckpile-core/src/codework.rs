//! `code-work add`'s git plumbing: cloning a repo's `base/` on demand, and
//! adding a worktree that tracks a branch already on `origin` or branches
//! one fresh when it isn't there yet.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// The branch `code-work add` derives from an item's key and the project's
/// `commit_prefix` — the tail after the key's last `-`, so `SGE-9876` with
/// `commit_prefix = "jr"` gives `jr-9876`, never the key lowercased whole.
pub fn derive_branch(id: &str, commit_prefix: &str) -> Result<String> {
    let (_, number) = id.rsplit_once('-').with_context(|| format!("{id}: no tiene la forma <prefijo>-<número>"))?;
    Ok(format!("{commit_prefix}-{number}"))
}

/// Clones `remote` into `dest`, on `principal_branch`, only if `dest`
/// doesn't already exist — never re-clones, never touches a clone that's
/// already there.
pub fn ensure_cloned(remote: &str, dest: &Path, principal_branch: &str) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    git(&["clone", "--branch", principal_branch, remote, &dest.to_string_lossy()])
}

/// Adds `worktree_path` as a worktree of `base`, on `branch` — tracking it
/// if `origin` already has it (retaking a review, continuing earlier work),
/// or branching it fresh from `from` when it doesn't.
pub fn add_worktree(base: &Path, worktree_path: &Path, branch: &str, from: &str) -> Result<()> {
    git_in(base, &["fetch", "origin"])?;
    let path = worktree_path.to_string_lossy();
    if remote_has_branch(base, branch)? {
        git_in(base, &["worktree", "add", "--track", "-b", branch, &path, &format!("origin/{branch}")])
    } else {
        git_in(base, &["worktree", "add", "-b", branch, &path, &format!("origin/{from}")])
    }
}

fn remote_has_branch(base: &Path, branch: &str) -> Result<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(base)
        .args(["show-ref", "--verify", "--quiet", &format!("refs/remotes/origin/{branch}")])
        .status()
        .with_context(|| format!("checking for origin/{branch} in {}", base.display()))?;
    Ok(status.success())
}

fn git(args: &[&str]) -> Result<()> {
    let status = Command::new("git").args(args).status().with_context(|| format!("running git {args:?}"))?;
    if !status.success() {
        bail!("git {args:?} failed with {status}");
    }
    Ok(())
}

fn git_in(dir: &Path, args: &[&str]) -> Result<()> {
    let status =
        Command::new("git").arg("-C").arg(dir).args(args).status().with_context(|| format!("running git {args:?} in {}", dir.display()))?;
    if !status.success() {
        bail!("git {args:?} in {} failed with {status}", dir.display());
    }
    Ok(())
}
