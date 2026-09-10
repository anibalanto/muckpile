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

/// Everything `show --local` needs that `list`/`status` don't: the body,
/// and a status that's allowed to be absent — a draft `new` just wrote
/// hasn't synced, and showing it is exactly the point of `--local`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullItem {
    pub id: String,
    pub item_type: String,
    pub title: String,
    pub status: Option<String>,
    pub parent: Option<String>,
    pub body: String,
}

/// Parses `path`'s frontmatter. The id and type come from the filename, not
/// the body — the same split `find_file` already relies on elsewhere.
pub fn read_summary(path: &Path) -> Result<ItemSummary> {
    let (id, item_type) = id_and_type(path)?;
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let parsed = parse_frontmatter(&text).with_context(|| format!("{}: no empieza con frontmatter", path.display()))?;

    Ok(ItemSummary {
        id,
        item_type,
        title: parsed.title.with_context(|| format!("{}: sin title en el frontmatter", path.display()))?,
        status: parsed.status.with_context(|| format!("{}: sin status en el frontmatter", path.display()))?,
        parent: parsed.parent,
    })
}

/// Like `read_summary`, but keeps the body and never requires a status.
pub fn read_full(path: &Path) -> Result<FullItem> {
    let (id, item_type) = id_and_type(path)?;
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_full(id, item_type, &text)
}

/// Like `read_full`, but from text already in hand rather than a path on
/// disk — `push` uses it on both the working copy of an item and a
/// git-committed snapshot of it (see `head_text`), neither of which is
/// necessarily what's on disk right now.
pub fn parse_full(id: String, item_type: String, text: &str) -> Result<FullItem> {
    let parsed = parse_frontmatter(text).with_context(|| format!("{id}.{item_type}.md: no empieza con frontmatter"))?;
    Ok(FullItem {
        id,
        item_type,
        title: parsed.title.with_context(|| "sin title en el frontmatter".to_string())?,
        status: parsed.status,
        parent: parsed.parent,
        body: parsed.body,
    })
}

/// What splitting an item file's frontmatter from its body yields — any of
/// the three fields may be absent, since only `title` is ever required, and
/// only by the readers above, not by this split itself.
struct Frontmatter {
    title: Option<String>,
    status: Option<String>,
    parent: Option<String>,
    body: String,
}

fn parse_frontmatter(text: &str) -> Result<Frontmatter> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        bail!("no frontmatter");
    }

    let mut title = None;
    let mut status = None;
    let mut parent = None;
    for line in lines.by_ref() {
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

    let body = lines.collect::<Vec<_>>().join("\n");
    Ok(Frontmatter { title, status, parent, body })
}

fn id_and_type(path: &Path) -> Result<(String, String)> {
    let name =
        path.file_name().and_then(|n| n.to_str()).with_context(|| format!("{}: nombre de archivo inválido", path.display()))?;
    let stem = name.strip_suffix(".md").with_context(|| format!("{name}: no es un .md"))?;
    let (id, item_type) = stem.split_once('.').with_context(|| format!("{name}: no tiene la forma <id>.<tipo>.md"))?;
    Ok((id.to_string(), item_type.to_string()))
}

/// A local item still waiting on its first sync — as `new` wrote it, with no
/// `status` because nothing has ever fetched one for it. `parent`, if
/// present, may itself name another slug in the same batch rather than a
/// real provider key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingItem {
    pub slug: String,
    pub item_type: String,
    pub title: String,
    pub parent: Option<String>,
    pub body: String,
}

/// Every `@slug.<type>.md` directly inside `view` — the mirror image of
/// `list_summaries`, which leaves these out for the opposite reason: there's
/// no `status` yet to compare against the provider with, but there's a
/// title to search or create with.
pub fn list_pending(view: &Path) -> Result<Vec<PendingItem>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(view).with_context(|| format!("reading {}", view.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(slug) = TYPES.iter().find_map(|t| name.strip_suffix(&format!(".{t}.md"))) else { continue };
        if !is_unassigned(slug) {
            continue;
        }
        let (id, item_type) = id_and_type(&path)?;
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let parsed = parse_frontmatter(&text).with_context(|| format!("{}: no empieza con frontmatter", path.display()))?;
        out.push(PendingItem {
            slug: id,
            item_type,
            title: parsed.title.with_context(|| format!("{}: sin title en el frontmatter", path.display()))?,
            parent: parsed.parent,
            body: parsed.body,
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
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
