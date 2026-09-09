//! Finding a project's root from wherever the shell is standing, what that
//! position counts as, and reading the project's shared configuration.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One repo of a (possibly multi-repo) project: where `base/<name>/` clones
/// from, and which branch is its principal one.
#[derive(Debug, Clone, Deserialize)]
pub struct RepoConfig {
    pub remote: String,
    pub branch: String,
}

/// A project's shared configuration — `<project>/muckpile.toml`. Nothing
/// personal lives here: no email, no token, nothing that differs by machine.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub provider: String,
    pub jira_base_url: String,
    pub jira_project_key: String,
    #[serde(default)]
    pub jira_board_id: Option<u64>,
    pub commit_prefix: String,
    #[serde(default)]
    pub repos: BTreeMap<String, RepoConfig>,
    #[serde(default)]
    pub item_type: BTreeMap<String, String>,
}

/// Reads `<root>/muckpile.toml`.
pub fn load_project_config(root: &Path) -> Result<ProjectConfig> {
    let path = root.join("muckpile.toml");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Walks up from `start` looking for a directory holding `.muckpile/` — the
/// marker of a project root, one ledger per project.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".muckpile").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// What a directory, relative to its project root, counts as. The three
/// reserved names are `base/`, `backlog/`, `to-work/` — nothing else lives
/// loose at the root, so every other position is a fixed number of steps
/// into one of the three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Root,
    /// Anywhere under `base/`, at any depth — a repo clone, or inside one.
    Base,
    /// `backlog/` itself.
    Backlog,
    /// `backlog/sprint/` itself.
    BacklogSprint,
    /// Inside a specific sprint's folder, at any depth.
    SprintView,
    /// `to-work/` itself.
    ToWorkRoot,
    /// Inside a specific working view, at any depth — including its
    /// `code-work/<repo>/` worktrees.
    WorkView,
    /// Not under any of the three reserved names.
    Elsewhere,
}

pub fn classify(root: &Path, cwd: &Path) -> Position {
    let rel = cwd.strip_prefix(root).unwrap_or(cwd);
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    match parts.as_slice() {
        [] => Position::Root,
        [first, ..] if first == "base" => Position::Base,
        [b, s] if b == "backlog" && s == "sprint" => Position::BacklogSprint,
        [b] if b == "backlog" => Position::Backlog,
        [b, s, _, ..] if b == "backlog" && s == "sprint" => Position::SprintView,
        [t] if t == "to-work" => Position::ToWorkRoot,
        [t, _, ..] if t == "to-work" => Position::WorkView,
        _ => Position::Elsewhere,
    }
}

/// Fails unless `cwd` is exactly the project root — the invariant `to-work`
/// needs: with several repos possible under `base/`, there's no longer a
/// default worktree to guess from anywhere else.
pub fn require_root(root: &Path, cwd: &Path) -> Result<()> {
    if classify(root, cwd) == Position::Root {
        return Ok(());
    }
    let rel = cwd.strip_prefix(root).unwrap_or(cwd);
    bail!("to-work corre en la raíz del proyecto, no en {}", rel.display());
}
