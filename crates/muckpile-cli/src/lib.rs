//! The `muckpile` binary's commands, as plain functions over a project root
//! and a working directory. `main.rs` only parses argv and prints; every
//! rule that needs a test lives here instead.

use anyhow::{bail, Context, Result};
use muckpile_core::body::adf_to_body;
use muckpile_core::is_valid_id;
use muckpile_core::project::{classify, require_root, Position, ProjectConfig};
use muckpile_provider::provider::Provider;
use std::path::{Path, PathBuf};

/// Assembles a working view: `<root>/to-work/<id>/`, empty. Fetching the
/// item is `pull`'s job — see below — and this doesn't call it: a fresh
/// view and an update to one are two different moments to fail at.
pub fn to_work(root: &Path, cwd: &Path, id: &str) -> Result<PathBuf> {
    require_root(root, cwd)?;
    if !is_valid_id(id) {
        bail!("{id}: no es un id válido");
    }

    let view = root.join("to-work").join(id);
    if view.exists() {
        bail!("to-work/{id}: ya existe");
    }
    std::fs::create_dir_all(&view).with_context(|| format!("creating {}", view.display()))?;
    Ok(view)
}

/// Fetches one item and writes it as `<id>.<type>.md` into the `to-work/`
/// view `cwd` stands exactly in. With no `id`, it's the view's own anchor —
/// the same convention `worklist` already uses. Given one, it's a related
/// item added to that same view, not a new one. A sprint or a query are a
/// different call this doesn't cover yet.
pub fn pull(root: &Path, cwd: &Path, id: Option<&str>, provider: &dyn Provider, config: &ProjectConfig) -> Result<PathBuf> {
    let own_id = view_id(root, cwd)?;
    let id = match id {
        Some(id) => {
            if !is_valid_id(id) {
                bail!("{id}: no es un id válido");
            }
            id.to_string()
        }
        None => own_id,
    };
    let item = provider.item(&id)?;
    let item_type = muckpile_type_of(config, &item.jira_type)?;

    let mut text = String::from("---\n");
    text.push_str(&format!("title: {}\n", item.title));
    text.push_str(&format!("status: {}\n", item.status));
    if let Some(parent) = &item.parent {
        text.push_str(&format!("parent: {parent}\n"));
    }
    text.push_str("---\n");
    if let Some(adf) = &item.body_adf {
        text.push_str(&adf_to_body(adf)?);
    }

    let path = cwd.join(format!("{id}.{item_type}.md"));
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// The id `pull` targets when called with none: the name of the `to-work/`
/// view `cwd` stands exactly in, not one it's merely nested under —
/// `code-work/<repo>/` inside it is a different repo's concern, not an item.
fn view_id(root: &Path, cwd: &Path) -> Result<String> {
    if classify(root, cwd) != Position::WorkView {
        bail!("pull sin argumento corre parado en una vista de to-work/<id>/");
    }
    let rel = cwd.strip_prefix(root).unwrap_or(cwd);
    let mut parts = rel.components();
    parts.next(); // "to-work"
    let id = parts.next().unwrap().as_os_str().to_string_lossy().into_owned();
    if parts.next().is_some() {
        bail!("pull sin argumento corre en la raíz de la vista, no en {}", rel.display());
    }
    Ok(id)
}

/// The muckpile type whose `muckpile.toml` mapping names `jira_type` — the
/// reverse of the table decision 9 declares (muckpile type -> provider type).
fn muckpile_type_of(config: &ProjectConfig, jira_type: &str) -> Result<String> {
    config
        .item_type
        .iter()
        .find(|(_, v)| v.as_str() == jira_type)
        .map(|(k, _)| k.clone())
        .with_context(|| format!("{jira_type}: sin tipo de item para él en muckpile.toml"))
}
