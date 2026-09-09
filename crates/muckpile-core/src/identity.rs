//! `~/.config/muckpile/identity.toml` — the machine-local half of a
//! project's configuration. Never shared, never in a repo: which Jira
//! account runs `muckpile` here, and the name of the environment variable
//! holding its token — never the token itself.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Identity {
    pub jira_email: String,
    pub jira_token_env: String,
}

#[derive(Debug, Deserialize)]
struct IdentityFile {
    #[serde(default)]
    projects: BTreeMap<String, Identity>,
}

/// Reads `path` and returns `project`'s identity.
pub fn load_identity(path: &Path, project: &str) -> Result<Identity> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file: IdentityFile = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    file.projects.get(project).cloned().with_context(|| format!("{project}: sin identidad en {}", path.display()))
}
