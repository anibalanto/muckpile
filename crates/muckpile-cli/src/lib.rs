//! The `muckpile` binary's commands, as plain functions over a project root
//! and a working directory. `main.rs` only parses argv and prints; every
//! rule that needs a test lives here instead.

use anyhow::{bail, Context, Result};
use muckpile_core::body::adf_to_body;
use muckpile_core::is_valid_id;
use muckpile_core::project::{classify, require_root, Position, ProjectConfig};
use muckpile_core::states::write_states_cache;
use muckpile_provider::provider::{Provider, Sprint};
use muckpile_provider::transition::{transition as provider_transition, Outcome};
use std::collections::BTreeMap;
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

/// What `sprint_fetch` did: the slugs it created, the slugs it removed
/// (empty folders it had left, for a sprint no longer open), and how many
/// sprints are open right now regardless of which of those were new.
#[derive(Debug)]
pub struct SprintFetch {
    pub open: usize,
    pub created: Vec<String>,
    pub removed: Vec<String>,
}

/// One empty folder per open sprint, under `backlog/sprint/<slug>/` — never
/// populated, only a place `pull` and the shell's tab-completion can find.
/// Never deletes a folder with anything in it; only one it left empty whose
/// sprint isn't open any more.
pub fn sprint_fetch(root: &Path, provider: &dyn Provider, config: &ProjectConfig) -> Result<SprintFetch> {
    let board_id = config.jira_board_id.with_context(|| "muckpile.toml no tiene jira_board_id".to_string())?;
    let sprints = provider.open_sprints(board_id)?;
    let slugs: Vec<String> = sprints.iter().map(|s| slugify(&legible_name(s))).collect();

    let dir = root.join("backlog/sprint");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut created = Vec::new();
    for slug in &slugs {
        let path = dir.join(slug);
        if !path.exists() {
            std::fs::create_dir(&path).with_context(|| format!("creating {}", path.display()))?;
            created.push(slug.clone());
        }
    }

    let mut removed = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if slugs.contains(&name) {
            continue;
        }
        if std::fs::read_dir(entry.path())?.next().is_none() {
            std::fs::remove_dir(entry.path())?;
            removed.push(name);
        }
    }

    Ok(SprintFetch { open: sprints.len(), created, removed })
}

/// A sprint's own name, as the shell can name a folder with: spaces become
/// `_`, nothing else changes — the number the project already puts in the
/// name (`"22 Las vistas"`) travels as-is, not reinvented as an id.
fn slugify(name: &str) -> String {
    name.replace(' ', "_")
}

/// `sprint.name`, with a provider-side truncation made legible. Measured on
/// the real ACC board: several open sprints have their `name` already cut
/// to 29 characters with a trailing `…` on the provider's own record — the
/// sprint number in front of it already tells two sprints apart, so what's
/// missing isn't disambiguation, it's not ending a folder name in a glyph
/// that carries nothing. The `…` is replaced by the sprint's creation date.
fn legible_name(sprint: &Sprint) -> String {
    match sprint.name.strip_suffix('…') {
        Some(truncated) => format!("{truncated} {}", date_only(&sprint.created)),
        None => sprint.name.clone(),
    }
}

/// The date part of an ISO-8601 timestamp — Jira's `createdDate` is always
/// ASCII up to that point, so byte slicing is safe.
fn date_only(iso: &str) -> &str {
    &iso[..10.min(iso.len())]
}

/// Replaces `start`/`done`/`close`/`drop` (decision 8): fires the workflow's
/// own transition leading to `target_status`, deciding by `to` — never by
/// guessing whether a transition's own name is the status it leads to. The
/// deciding logic lives in `muckpile-provider`; this only adds the id check
/// every other command already applies to an argument coming from argv.
pub fn transition(id: &str, target_status: &str, provider: &dyn Provider) -> Result<Outcome> {
    if !is_valid_id(id) {
        bail!("{id}: no es un id válido");
    }
    provider_transition(provider, id, target_status)
}

/// Lists the project's workflow statuses, live, and caches `{name ->
/// category}` at `<root>/<project>.states.toml` — so `--category` filters
/// (not this slice) never have to ask the provider again, and so a person
/// can copy a status name from the cache instead of typing it from memory.
pub fn states_discover(root: &Path, project: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<BTreeMap<String, String>> {
    let statuses = provider.project_statuses(&config.jira_project_key)?;
    let states: BTreeMap<String, String> = statuses.into_iter().map(|s| (s.name, s.category)).collect();
    let path = root.join(format!("{project}.states.toml"));
    write_states_cache(&path, &states)?;
    Ok(states)
}
