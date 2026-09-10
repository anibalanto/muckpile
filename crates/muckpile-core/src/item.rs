//! Reading back an item file's frontmatter — the format `pull` writes:
//! `title`, `status`, and an optional `parent`, ahead of the body. `list`
//! filters over this instead of asking the provider again.

use crate::{is_unassigned, TYPES};
use anyhow::{bail, Context, Result};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    pub id: String,
    pub item_type: String,
    pub title: String,
    pub status: String,
    pub parent: Option<String>,
}

/// Parses `path`'s frontmatter. The id and type come from the filename, not
/// the body — the same split `find_file` already relies on elsewhere.
pub fn read_summary(path: &Path) -> Result<ItemSummary> {
    let (id, item_type) = id_and_type(path)?;
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        bail!("{}: no empieza con frontmatter", path.display());
    }

    let mut title = None;
    let mut status = None;
    let mut parent = None;
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("title: ") {
            title = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("status: ") {
            status = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("parent: ") {
            parent = Some(v.to_string());
        }
    }

    Ok(ItemSummary {
        id,
        item_type,
        title: title.with_context(|| format!("{}: sin title en el frontmatter", path.display()))?,
        status: status.with_context(|| format!("{}: sin status en el frontmatter", path.display()))?,
        parent,
    })
}

fn id_and_type(path: &Path) -> Result<(String, String)> {
    let name =
        path.file_name().and_then(|n| n.to_str()).with_context(|| format!("{}: nombre de archivo inválido", path.display()))?;
    let stem = name.strip_suffix(".md").with_context(|| format!("{name}: no es un .md"))?;
    let (id, item_type) = stem.split_once('.').with_context(|| format!("{name}: no tiene la forma <id>.<tipo>.md"))?;
    Ok((id.to_string(), item_type.to_string()))
}

/// Every `<id>.<type>.md` directly inside `view` that has actually synced —
/// never recursing, so a worktree under `code-work/` or a question's
/// `_data/` never leaks in as an item, and never a `@slug` still waiting on
/// its first `push`: it has no status yet, nothing to list or compare
/// against the provider by.
pub fn list_summaries(view: &Path) -> Result<Vec<ItemSummary>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(view).with_context(|| format!("reading {}", view.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(id) = TYPES.iter().find_map(|t| name.strip_suffix(&format!(".{t}.md"))) else { continue };
        if is_unassigned(id) {
            continue;
        }
        out.push(read_summary(&path)?);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
