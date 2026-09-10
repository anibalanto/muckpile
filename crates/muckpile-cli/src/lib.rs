//! The `muckpile` binary's commands, as plain functions over a project root
//! and a working directory. `main.rs` only parses argv and prints; every
//! rule that needs a test lives here instead.

use anyhow::{bail, Context, Result};
use muckpile_core::body::{self, adf_to_body, body_to_adf, Filtered, JiraAdfMarkdownFilter, Loss};
use muckpile_core::codework::{add_worktree, derive_branch, ensure_cloned};
use muckpile_core::item::{self, list_summaries, parse_full, read_full, ItemSummary};
use muckpile_core::project::{classify, require_root, Position, ProjectConfig};
use muckpile_core::states::write_states_cache;
use muckpile_core::{
    commit_paths, find_file, head_text, is_valid_id, read_frontmatter_refs, resolve_batch, slugify_title, topo_order, MARKER, TYPES,
};
use muckpile_provider::link::{link as provider_link, Outcome as LinkOutcome};
use muckpile_provider::provider::{Item, Provider, Sprint};
use muckpile_provider::transition::{transition as provider_transition, Outcome};
use std::collections::{BTreeMap, HashMap, HashSet};
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

/// Writes `@<slug>.<type>.md` into `dir` — no network, no provider: the id
/// is local until the first `push` resolves it (decision 4). `blocks`
/// declares the one relation a fresh item can carry, `relation.blocks`
/// (decision 7's `question`, though nothing here restricts it to that type).
pub fn new(dir: &Path, item_type: &str, title: &str, blocks: Option<&str>) -> Result<PathBuf> {
    if !TYPES.contains(&item_type) {
        bail!("{item_type}: tipo desconocido — {}", TYPES.join(", "));
    }
    if let Some(blocks) = blocks {
        if !is_valid_id(blocks) {
            bail!("{blocks}: no es un id válido");
        }
    }

    let slug = format!("{MARKER}{}", slugify_title(title));
    let path = dir.join(format!("{slug}.{item_type}.md"));
    if path.exists() {
        bail!("{slug}.{item_type}.md: ya existe");
    }

    let mut text = format!("---\ntitle: {title}\n");
    if let Some(blocks) = blocks {
        text.push_str(&format!("relation.blocks: {blocks}\n"));
    }
    text.push_str("---\n");
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// What `pull` brought for one item: where it landed, and every reason its
/// body can't be edited locally — none when it can. Said the moment the body
/// comes down, not later when someone already edited it.
#[derive(Debug)]
pub struct Pulled {
    pub path: PathBuf,
    pub losses: Vec<Loss>,
}

/// Fetches one item and writes it as `<id>.<type>.md` into the `to-work/`
/// view `cwd` stands exactly in. With no `id`, it's the view's own anchor —
/// the same convention `worklist` already uses. Given one, it's a related
/// item added to that same view, not a new one. A sprint or a query are a
/// different call this doesn't cover yet.
pub fn pull(root: &Path, cwd: &Path, id: Option<&str>, provider: &dyn Provider, config: &ProjectConfig) -> Result<Pulled> {
    let own_id = work_view_id(root, cwd)?;
    let id = match id {
        Some(id) => {
            if !is_valid_id(id) {
                bail!("{id}: no es un id válido");
            }
            id.to_string()
        }
        None => own_id,
    };
    fetch_and_commit(cwd, &id, provider, config)
}

/// Fetches `id` and writes/commits it into `dir` in pulled form — the tail
/// `pull` runs for its own argument, and what resolving a pending `@slug`
/// runs once it has a real key, so that a freshly created or found item
/// ends up with the same commit `push`'s compare-and-swap already knows how
/// to read.
fn fetch_and_commit(dir: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<Pulled> {
    let item = provider.item(id)?;
    let item_type = muckpile_type_of(config, &item.jira_type)?;
    let (text, losses) = render_pulled_text(&item)?;

    let filename = format!("{id}.{item_type}.md");
    let path = dir.join(&filename);
    std::fs::write(&path, &text).with_context(|| format!("writing {}", path.display()))?;

    // The commit is what lets `push` later ask "did the provider change
    // since I last asked?" without a state file of its own — see
    // `commit_paths`.
    commit_paths(dir, &[&filename], &format!("pull {id}"))?;
    Ok(Pulled { path, losses })
}

/// The exact text `pull` writes for a fetched item, and why its body can't
/// be edited locally — also what `push` uses to re-derive "what a fresh pull
/// would say right now", to compare against the last one it actually
/// committed.
fn render_pulled_text(item: &Item) -> Result<(String, Vec<Loss>)> {
    let mut text = String::from("---\n");
    text.push_str(&format!("title: {}\n", item.title));
    text.push_str(&format!("status: {}\n", item.status));
    if let Some(parent) = &item.parent {
        text.push_str(&format!("parent: {parent}\n"));
    }
    text.push_str("---\n");
    let body = match &item.body_adf {
        Some(adf) => JiraAdfMarkdownFilter::filter(adf)?,
        None => Filtered::default(),
    };
    text.push_str(&body.markdown);
    Ok((text, body.losses))
}

/// The id of the `to-work/<id>/` view `cwd` stands exactly in, not one it's
/// merely nested under — `code-work/<repo>/` inside it is a different
/// repo's concern, not the view's own item. Shared by every command that
/// only makes sense run from inside a view's own root.
fn work_view_id(root: &Path, cwd: &Path) -> Result<String> {
    if classify(root, cwd) != Position::WorkView {
        bail!("corré esto parado en una vista de to-work/<id>/");
    }
    let rel = cwd.strip_prefix(root).unwrap_or(cwd);
    let mut parts = rel.components();
    parts.next(); // "to-work"
    let id = parts.next().unwrap().as_os_str().to_string_lossy().into_owned();
    if parts.next().is_some() {
        bail!("corré esto en la raíz de la vista, no en {}", rel.display());
    }
    Ok(id)
}

/// Adds `code-work/<repo>/` as a worktree of `<root>/base/<repo>/`, cloning
/// the latter on demand — one repo at a time, run from inside the
/// `to-work/<id>/` view it belongs to. `from` overrides the starting point
/// for a fresh branch (a hotfix off `rc-??`); `branch` overrides the derived
/// name entirely (a split front/back, or one that already exists under
/// another name) — the two are independent, each optional on its own.
pub fn code_work_add(
    root: &Path,
    cwd: &Path,
    repo_name: &str,
    from: Option<&str>,
    branch_override: Option<&str>,
    config: &ProjectConfig,
) -> Result<PathBuf> {
    let id = work_view_id(root, cwd)?;
    let repo_config = config.repos.get(repo_name).with_context(|| format!("{repo_name}: no está en muckpile.toml"))?;

    let base = root.join("base").join(repo_name);
    ensure_cloned(&repo_config.remote, &base, &repo_config.branch)?;

    let branch = match branch_override {
        Some(b) => b.to_string(),
        None => derive_branch(&id, &config.commit_prefix)?,
    };
    let from_branch = from.unwrap_or(&repo_config.branch);

    let worktree_path = cwd.join("code-work").join(repo_name);
    if worktree_path.exists() {
        bail!("code-work/{repo_name}: ya existe");
    }

    add_worktree(&base, &worktree_path, &branch, from_branch)?;
    Ok(worktree_path)
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

/// Replaces `depends`/`blocks` as muckpile's own vocabulary (decision 8,
/// extended from status to relationships): `phrase` is one of the
/// provider's own — the deciding-and-firing logic lives in
/// `muckpile-provider`; this only adds the id checks every other command
/// already applies to arguments coming from argv.
pub fn link(a: &str, phrase: &str, b: &str, provider: &dyn Provider) -> Result<LinkOutcome> {
    if !is_valid_id(a) {
        bail!("{a}: no es un id válido");
    }
    if !is_valid_id(b) {
        bail!("{b}: no es un id válido");
    }
    provider_link(provider, a, phrase, b)
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

/// What `list` narrows by — each present field is one more AND clause.
#[derive(Debug, Default)]
pub struct ListFilter<'a> {
    pub state: Option<&'a str>,
    pub category: Option<&'a str>,
    pub parent: Option<&'a str>,
}

/// Items already pulled into `view`, filtered without asking the provider
/// again — `state` compares the provider's own string, `category` looks it
/// up in `categories` (the cache `states discover` wrote; a status the cache
/// doesn't hold can't be claimed to match), `parent` compares by id.
pub fn list(view: &Path, filter: &ListFilter, categories: &BTreeMap<String, String>) -> Result<Vec<ItemSummary>> {
    let mut items = list_summaries(view)?;
    if let Some(state) = filter.state {
        items.retain(|i| i.status == state);
    }
    if let Some(category) = filter.category {
        items.retain(|i| categories.get(&i.status).is_some_and(|c| c == category));
    }
    if let Some(parent) = filter.parent {
        items.retain(|i| i.parent.as_deref() == Some(parent));
    }
    Ok(items)
}

/// One item's local fields against the provider's current ones — never the
/// body (that's canonicity's call, decision 10, and `push`'s problem, not
/// this read-only comparison's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStatus {
    pub id: String,
    pub changed: bool,
    pub local_title: String,
    pub remote_title: String,
    pub local_status: String,
    pub remote_status: String,
    pub local_parent: Option<String>,
    pub remote_parent: Option<String>,
}

/// Compares every item already pulled into `view` against the provider,
/// live — never writes, local or remote. What `push` still needs on top of
/// this is deciding what to do about a difference; this only finds one.
pub fn status(view: &Path, provider: &dyn Provider) -> Result<Vec<ItemStatus>> {
    list_summaries(view)?
        .into_iter()
        .map(|local| {
            let remote = provider.item(&local.id)?;
            Ok(ItemStatus {
                changed: local.title != remote.title || local.status != remote.status || local.parent != remote.parent,
                id: local.id,
                local_title: local.title,
                remote_title: remote.title,
                local_status: local.status,
                remote_status: remote.status,
                local_parent: local.parent,
                remote_parent: remote.parent,
            })
        })
        .collect()
}

/// What `push` did with one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushOutcome {
    pub id: String,
    pub result: PushResult,
}

/// What `push` found for one item, resolved down to a single case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    /// The file matches the last `pull` this view recorded for it — nothing
    /// to send.
    Unchanged,
    /// No commit exists for this item in this view yet, so there's nothing
    /// to compare the provider's current state against — refused rather
    /// than guessed.
    NeverPulled,
    /// The provider moved since the last `pull` this view recorded —
    /// refused rather than overwritten.
    Stale,
    /// The compare-and-swap cleared; what was actually sent. `body_refused`
    /// says why when the body changed locally but wasn't safe to send —
    /// title is judged on its own and can be `true` independently.
    Written { title: bool, body: bool, body_refused: Option<BodyRefused> },
    /// A pending `@slug` got a real id this run — found on the provider, or
    /// created there. `PushOutcome::id` for this case is still the original
    /// slug: the file has already moved to `id.<type>.md` by the time this
    /// comes back.
    Resolved { id: String, created: bool },
    /// A pending `@slug` couldn't be resolved this run — searching or
    /// creating it failed, or it depends on another pending item that
    /// failed first. The file is left exactly as it was, still pending.
    ResolveFailed(String),
}

/// A body edit `push` wouldn't send: why the provider's body can't be
/// written back through markdown, and what sending the draft would do to it
/// — for a person, or anything else working on the provider directly, to
/// apply there with the loss already in view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyRefused {
    pub losses: Vec<Loss>,
    pub diff: String,
}

/// First resolves every pending `@slug` the view carries — search or
/// create, then rename, rewrite references, and settle a fresh baseline for
/// it, same as a `pull` — and then sends every item that already had a real
/// id whose title or body changed since its own last `pull`, refusing
/// anything the provider moved on since then. Status and parent never
/// travel through the second half — nothing here writes a status directly,
/// and nothing re-parents an item that already had a key before this run —
/// so a commit it makes there always re-renders those two straight from the
/// provider, discarding any local edit to a field this never reads.
pub fn push(view: &Path, provider: &dyn Provider, config: &ProjectConfig) -> Result<Vec<PushOutcome>> {
    let mut outcomes = resolve_pending(view, provider, config)?;
    let just_resolved: HashSet<String> = outcomes
        .iter()
        .filter_map(|o| match &o.result {
            PushResult::Resolved { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();

    for local in list_summaries(view)? {
        if just_resolved.contains(&local.id) {
            // Its baseline was just committed fresh — nothing this same run
            // could have edited yet.
            continue;
        }
        outcomes.push(push_one(view, &local, provider)?);
    }
    Ok(outcomes)
}

/// Resolves every pending `@slug` directly under `view`: searches by exact
/// title before creating, so a retry never duplicates; a `parent` naming
/// another pending item in the same batch is translated to that item's
/// freshly resolved id first, since the provider has to already know about
/// a parent before it can be named on creation. Processed in topological
/// order over the batch's own internal references — `parent` and any
/// `relation.*` — so a dependency is always resolved before what depends on
/// it is attempted, and never attempted at all once its dependency failed.
fn resolve_pending(view: &Path, provider: &dyn Provider, config: &ProjectConfig) -> Result<Vec<PushOutcome>> {
    let pending = item::list_pending(view)?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    let slugs: Vec<String> = pending.iter().map(|p| p.slug.clone()).collect();
    let order = topo_order(view, &slugs)?;
    let by_slug: HashMap<&str, &item::PendingItem> = pending.iter().map(|p| (p.slug.as_str(), p)).collect();

    let mut resolved: HashMap<String, String> = HashMap::new();
    let mut outcomes = Vec::new();

    for slug in &order {
        let pending_item = by_slug[slug.as_str()];

        let (path, _) = find_file(view, slug)?;
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let blocked_by = read_frontmatter_refs(&text).into_iter().find(|r| by_slug.contains_key(r.as_str()) && !resolved.contains_key(r));
        if let Some(blocker) = blocked_by {
            outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::ResolveFailed(format!("depende de {blocker}, que no se pudo resolver")) });
            continue;
        }

        let Some(jira_type) = config.item_type.get(&pending_item.item_type) else {
            outcomes.push(PushOutcome {
                id: slug.clone(),
                result: PushResult::ResolveFailed(format!("{}: sin tipo de item para él en muckpile.toml", pending_item.item_type)),
            });
            continue;
        };
        let real_parent = pending_item.parent.as_deref().map(|p| resolved.get(p).map(String::as_str).unwrap_or(p));

        match resolve_one(pending_item, jira_type, real_parent, provider, config) {
            Ok((id, created)) => {
                resolved.insert(slug.clone(), id.clone());
                outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::Resolved { id, created } });
            }
            Err(e) => outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::ResolveFailed(e.to_string()) }),
        }
    }

    if !resolved.is_empty() {
        // `git mv` needs its source tracked, and nothing commits a fresh
        // `@slug` draft before this — `new` writes it and stops there, the
        // same way `pull` used to. A no-op if it's already committed.
        for slug in resolved.keys() {
            let pending_item = by_slug[slug.as_str()];
            let filename = format!("{}.{}.md", pending_item.slug, pending_item.item_type);
            commit_paths(view, &[&filename], &format!("new {slug}"))?;
        }
        resolve_batch(view, &resolved)?;
        for id in resolved.values() {
            fetch_and_commit(view, id, provider, config)?;
        }
    }

    Ok(outcomes)
}

/// One pending item, already past the batch-dependency check: search by
/// title, and only create when nothing matches. A found item never sends
/// its draft body or parent — both only ever travel at creation, and
/// finding means something already existed before this run touched it.
fn resolve_one(
    pending: &item::PendingItem,
    jira_type: &str,
    real_parent: Option<&str>,
    provider: &dyn Provider,
    config: &ProjectConfig,
) -> Result<(String, bool)> {
    if let Some(id) = provider.find_by_title(&config.jira_project_key, jira_type, &pending.title)? {
        return Ok((id, false));
    }
    let body_adf = if pending.body.trim().is_empty() { None } else { Some(body_to_adf(&pending.body)?) };
    let id = provider.create_item(&config.jira_project_key, jira_type, &pending.title, real_parent, body_adf.as_deref())?;
    Ok((id, true))
}

fn push_one(view: &Path, local: &ItemSummary, provider: &dyn Provider) -> Result<PushOutcome> {
    let filename = format!("{}.{}.md", local.id, local.item_type);
    let outcome = |result| PushOutcome { id: local.id.clone(), result };

    let Some(head) = head_text(view, &filename)? else {
        return Ok(outcome(PushResult::NeverPulled));
    };

    let working_path = view.join(&filename);
    let working = std::fs::read_to_string(&working_path).with_context(|| format!("reading {}", working_path.display()))?;
    if working == head {
        return Ok(outcome(PushResult::Unchanged));
    }

    // Ask again before writing: what the provider has *right now*, rendered
    // the same way a fresh `pull` would, has to still match the last
    // commit — not the working copy, which is expected to differ by
    // exactly the edit this call is trying to send.
    let remote = provider.item(&local.id)?;
    let (remote_text, losses) = render_pulled_text(&remote)?;
    if remote_text != head {
        return Ok(outcome(PushResult::Stale));
    }

    let head_title = parse_full(local.id.clone(), local.item_type.clone(), &head)?.title;
    let (_, head_body) = body::split_frontmatter(&head);
    let (_, working_body) = body::split_frontmatter(&working);

    let title_changed = local.title != head_title;
    let body_changed = working_body != head_body;

    if title_changed {
        provider.update_title(&local.id, &local.title)?;
    }

    let mut body_sent = false;
    let mut body_refused = None;
    let mut sent_adf = None;
    if body_changed {
        // Canonicity is decided on the provider's ADF as it is right now —
        // just confirmed to still be what this view last pulled — and never
        // on the committed markdown, which can come back from its own trip
        // unchanged while the ADF behind it doesn't. Only a body read from
        // some ADF can have losses.
        if let Some(real) = remote.body_adf.as_deref().filter(|_| !losses.is_empty()) {
            body_refused = Some(BodyRefused { losses, diff: body::adf_diff(real, working_body)? });
        } else {
            let adf = body_to_adf(working_body)?;
            provider.update_body(&local.id, &adf)?;
            body_sent = true;
            sent_adf = Some(adf);
        }
    }

    // The new baseline is rendered fresh from the provider's own fields —
    // status and parent as they really are, title/body swapped in only for
    // what was actually confirmed sent. A refused body edit, or a hand-edit
    // to a field `push` doesn't manage, never gets to look committed.
    let committed = Item {
        jira_type: remote.jira_type.clone(),
        title: if title_changed { local.title.clone() } else { remote.title.clone() },
        status: remote.status.clone(),
        parent: remote.parent.clone(),
        body_adf: if body_sent { sent_adf } else { remote.body_adf.clone() },
    };
    let (new_text, _) = render_pulled_text(&committed)?;
    std::fs::write(&working_path, &new_text).with_context(|| format!("writing {}", working_path.display()))?;
    commit_paths(view, &[&filename], &format!("push {}", local.id))?;

    Ok(outcome(PushResult::Written { title: title_changed, body: body_sent, body_refused }))
}

/// What `show` prints — frontmatter plus body, live or local, and the
/// `<id>_data/` listing (decision 7) alongside either, since that directory
/// is local filesystem state regardless of where the rest came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Show {
    pub title: String,
    pub status: Option<String>,
    pub parent: Option<String>,
    pub body: String,
    pub data_files: Vec<String>,
}

/// `show`'s live path: the provider's current fields, body converted from
/// ADF the same way `pull` does.
pub fn show_live(dir: &Path, id: &str, provider: &dyn Provider) -> Result<Show> {
    if !is_valid_id(id) {
        bail!("{id}: no es un id válido");
    }
    let item = provider.item(id)?;
    let body = match &item.body_adf {
        Some(adf) => adf_to_body(adf)?,
        None => String::new(),
    };
    Ok(Show { title: item.title, status: Some(item.status), parent: item.parent, body, data_files: data_files(dir, id)? })
}

/// `show --local`: whatever's already on disk — a pulled item, or a draft
/// `new` wrote that hasn't synced — without asking the provider.
pub fn show_local(dir: &Path, id: &str) -> Result<Show> {
    let (path, _item_type) = find_file(dir, id)?;
    let full = read_full(&path)?;
    Ok(Show { title: full.title, status: full.status, parent: full.parent, body: full.body, data_files: data_files(dir, id)? })
}

/// The names under `<dir>/<id>_data/`, or empty when there's no such
/// directory — nothing writes into it yet (decision 7's `thread`/`files`
/// aren't implemented), so this only ever reports what a person put there.
fn data_files(dir: &Path, id: &str) -> Result<Vec<String>> {
    let data_dir = dir.join(format!("{id}_data"));
    if !data_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = std::fs::read_dir(&data_dir)
        .with_context(|| format!("reading {}", data_dir.display()))?
        .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_>>()?;
    names.sort();
    Ok(names)
}
