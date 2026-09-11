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
    pub item_type: BTreeMap<String, ItemType>,
    /// Named queries, in the provider's own query language: what
    /// `query/<name>/` holds.
    #[serde(default)]
    pub queries: BTreeMap<String, String>,
    /// Whether creating and editing on the provider goes without a person
    /// retyping a phrase at a terminal. Off unless the file says so.
    #[serde(default)]
    pub auto_update: bool,
    /// Whether `comment --ai` goes at all. Off unless the file says so.
    #[serde(default)]
    pub auto_comment: bool,
}

/// How one muckpile type lives on the provider: its issue type, and the
/// label that tells it apart when another muckpile type shares that issue
/// type. Written as a bare string when it needs no label.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "ItemTypeEntry")]
pub struct ItemType {
    pub jira_type: String,
    pub label: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ItemTypeEntry {
    Plain(String),
    Labeled {
        #[serde(rename = "type")]
        jira_type: String,
        label: String,
    },
}

impl From<ItemTypeEntry> for ItemType {
    fn from(entry: ItemTypeEntry) -> Self {
        match entry {
            ItemTypeEntry::Plain(jira_type) => ItemType { jira_type, label: None },
            ItemTypeEntry::Labeled { jira_type, label } => ItemType { jira_type, label: Some(label) },
        }
    }
}

impl From<&str> for ItemType {
    fn from(jira_type: &str) -> Self {
        ItemType { jira_type: jira_type.to_string(), label: None }
    }
}

impl ProjectConfig {
    /// The muckpile type an item of `jira_type` carrying `labels` is: the
    /// one whose label it carries, or else the one on that type with no
    /// label — `None` when neither exists.
    pub fn muckpile_type_of(&self, jira_type: &str, labels: &[String]) -> Option<&str> {
        let on_type = || self.item_type.iter().filter(|(_, t)| t.jira_type == jira_type);
        on_type()
            .find(|(_, t)| t.label.as_ref().is_some_and(|l| labels.contains(l)))
            .or_else(|| on_type().find(|(_, t)| t.label.is_none()))
            .map(|(name, _)| name.as_str())
    }
}

/// Reads `<root>/muckpile.toml`, refusing an `item_type` table that would
/// leave an item unable to say which muckpile type it is.
pub fn load_project_config(root: &Path) -> Result<ProjectConfig> {
    let path = root.join("muckpile.toml");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let config: ProjectConfig = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    check_item_types(&config.item_type).with_context(|| format!("{}", path.display()))?;
    Ok(config)
}

/// Among the muckpile types sharing one provider type, at most one goes
/// without a label, and no two share a label — otherwise an item of that
/// type couldn't say which of them it is.
fn check_item_types(item_types: &BTreeMap<String, ItemType>) -> Result<()> {
    let mut by_key: BTreeMap<(&str, Option<&str>), Vec<&str>> = BTreeMap::new();
    for (name, t) in item_types {
        by_key.entry((t.jira_type.as_str(), t.label.as_deref())).or_default().push(name);
    }
    for ((jira_type, label), names) in by_key {
        if names.len() > 1 {
            let names = names.join(&crate::msg!("words.and"));
            match label {
                None => bail!(crate::msg!("config.item_type.clash_unlabeled", names, jira_type)),
                Some(label) => bail!(crate::msg!("config.item_type.clash_same_label", names, jira_type, label)),
            }
        }
    }
    Ok(())
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

/// What a directory, relative to its project root, counts as. The four
/// reserved names are `base/`, `backlog/`, `to-work/`, `query/` — nothing
/// else lives loose at the root, so every other position is a fixed number
/// of steps into one of the four.
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
    /// `query/` itself.
    QueryRoot,
    /// Inside a specific query's folder, at any depth.
    QueryView,
    /// `to-work/` itself.
    ToWorkRoot,
    /// Inside a specific working view, at any depth — including its
    /// `code-work/<repo>/` worktrees.
    WorkView,
    /// Not under any of the four reserved names.
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
        [q] if q == "query" => Position::QueryRoot,
        [q, _, ..] if q == "query" => Position::QueryView,
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
    bail!(crate::msg!("to_work.not_at_root", path = rel.display()));
}
