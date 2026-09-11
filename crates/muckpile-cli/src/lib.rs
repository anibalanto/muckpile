//! The `muckpile` binary's commands, as plain functions over a project root
//! and a working directory. `main.rs` only parses argv and prints; every
//! rule that needs a test lives here instead.

use anyhow::{bail, Context, Result};
use muckpile_core::body::{self, body_to_adf, cards_to_file_links, cited_keys, file_links_to_cards, Filtered, JiraAdfMarkdownFilter, Loss};
use muckpile_core::codework::{add_worktree, derive_branch, ensure_cloned};
use muckpile_core::item::{self, list_summaries, parse_full, read_full, ItemSummary};
use muckpile_core::ledger::{self, Rebase};
use muckpile_core::project::{classify, require_root, ItemType, Position, ProjectConfig};
use muckpile_core::states::write_states_cache;
use muckpile_core::{commit_paths, msg, find_file, is_valid_id, read_frontmatter_refs, read_relations, rename_one, rewrite_type_references, slugify_title, topo_order, MARKER, TYPES};
use muckpile_provider::link::{link as provider_link, unlink as provider_unlink, Outcome as LinkOutcome, UnlinkOutcome};
use muckpile_provider::provider::{Attachment, Comment, Item, ItemLink, Provider, Sprint};
use muckpile_provider::transition::{transition as provider_transition, Outcome};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Every command, in the groups `help` lists them under: the group's name,
/// and each command's first word as argv spells it. How it's run and what
/// it does are messages, `help.usage.<command>` and `help.what.<command>`.
const COMMANDS: &[(&str, &[&str])] = &[
    ("project", &["init", "to-work", "code-work", "sprint", "states"]),
    ("read", &["show", "list", "status"]),
    ("sync", &["new", "pull", "push"]),
    ("write", &["title", "transition", "parent", "link", "unlink", "comment", "attach"]),
];

/// What `muckpile --help` prints: every command, group by group, each one
/// on its own line with what it does under it.
pub fn help() -> String {
    let mut text = format!("{}\n", msg!("help.header"));
    for (group, commands) in COMMANDS {
        text.push_str(&format!("\n{}\n", msg!(&format!("help.group.{group}"))));
        for command in *commands {
            text.push_str(&format!("  {}\n      {}\n", msg!(&format!("help.usage.{command}")), msg!(&format!("help.what.{command}"))));
        }
    }
    text.push_str(&format!("\n{}\n", msg!("help.footer")));
    text
}

/// How `command` is run, in one line — `None` when there's no such
/// command.
pub fn usage_of(command: &str) -> Option<String> {
    let known = COMMANDS.iter().flat_map(|(_, commands)| commands.iter()).find(|c| **c == command)?;
    Some(msg!("usage.one", usage = msg!(&format!("help.usage.{known}"))))
}

/// Makes a project, `<multitask>/<name>/`: its ledger, a `muckpile.toml`
/// to fill in, and the four reserved folders. The only command that makes
/// a ledger — every other one refuses outside a project that has one.
pub fn init(multitask: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
        bail!(msg!("init.bad_name", name = format!("{name:?}")));
    }
    let project = multitask.join(name);
    if project.join(ledger::LEDGER).exists() {
        bail!(msg!("init.already_a_project", name));
    }
    for folder in ["base", "backlog/sprint", "to-work", "query"] {
        std::fs::create_dir_all(project.join(folder)).with_context(|| format!("creating {}/{folder}", project.display()))?;
    }
    ledger::init(&project)?;
    let config = project.join("muckpile.toml");
    if !config.exists() {
        std::fs::write(&config, CONFIG_TEMPLATE).with_context(|| format!("writing {}", config.display()))?;
    }
    Ok(project)
}

/// What `init` leaves to fill in: the shape of the file, with placeholders
/// where only the person knows the value.
const CONFIG_TEMPLATE: &str = r#"provider = "jira-rest"
jira_base_url = "https://INSTANCIA.atlassian.net"
jira_project_key = "CLAVE"
# jira_board_id = 0
commit_prefix = "prefijo"

# true: crear y editar en el proveedor sin que una persona escriba la frase
auto_update = false
# true: comment --ai manda sin pedir nada
auto_comment = false

# [repos.nombre]
# remote = "git@host:grupo/repo.git"
# branch = "main"

[item_type]
task = "Tarea"
user-story = "Historia"
epic = "Epic"
question = { type = "Tarea", label = "question" }
"#;

/// Assembles a working view: `<root>/to-work/<id>/`, a worktree of the
/// project's ledger, empty — what `to-work --empty` leaves. By default the
/// command also brings the item: `to_work_and_pull`.
pub fn to_work(root: &Path, cwd: &Path, id: &str) -> Result<PathBuf> {
    require_root(root, cwd)?;
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    ledger::open_view(root, &format!("to-work/{id}"))
}

/// `to-work <id>` as it runs by default: the view, and the item pulled
/// into it — its file and its `_data/`. When the item can't be brought, the
/// view just made is closed again, so a mistyped id leaves nothing behind.
pub fn to_work_and_pull(root: &Path, cwd: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<Pulled> {
    let view = to_work(root, cwd, id)?;
    match pull(root, &view, None, provider, config) {
        Ok(pulled) => Ok(pulled),
        Err(e) => {
            let _ = ledger::close_empty_view(root, &format!("to-work/{id}"));
            Err(e)
        }
    }
}

/// Writes `@<slug>.<type>.md` into `dir` — no network, no provider: the id
/// is local until the first `push` resolves it. `blocks` declares the one
/// relation a fresh item can carry, `relation.blocks` (the one a `question`
/// exists to declare, though nothing here restricts it to that type).
/// `parent` names the item it hangs from — a real id, or another draft's
/// `@slug`, which `push` translates once that draft has its id. Both travel
/// with the rest of the header when the item is created, and never after.
pub fn new(dir: &Path, item_type: &str, title: &str, parent: Option<&str>, blocks: Option<&str>) -> Result<PathBuf> {
    if !TYPES.contains(&item_type) {
        bail!(msg!("new.unknown_type", item_type, known = TYPES.join(", ")));
    }
    if let Some(parent) = parent {
        if !is_valid_id(parent) {
            bail!(msg!("id.invalid", id = parent));
        }
    }
    if let Some(blocks) = blocks {
        if !is_valid_id(blocks) {
            bail!(msg!("id.invalid", id = blocks));
        }
    }

    let slug = format!("{MARKER}{}", slugify_title(title));
    let path = dir.join(format!("{slug}.{item_type}.md"));
    if path.exists() {
        bail!(msg!("path.exists", path = format!("{slug}.{item_type}.md")));
    }

    let mut text = format!("---\ntitle: {title}\n");
    if let Some(parent) = parent {
        text.push_str(&format!("parent: {parent}\n"));
    }
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
    /// The other views of the project, on this machine, that already hold
    /// the item — asked of the disk right here, never of a shared record.
    pub also_in: Vec<String>,
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
                bail!(msg!("id.invalid", id));
            }
            id.to_string()
        }
        None => own_id,
    };
    ready_to_sync(cwd)?;
    let mut pulled = record_item(cwd, &id, provider, config, &format!("pull {id}"))?;
    rebase_or_explain(cwd)?;
    pulled.also_in = other_views_holding(root, cwd, &id)?;
    Ok(pulled)
}

/// What `pull` of a sprint's or a query's view brought: every item it
/// holds, and the ids of the ones that went — out of the sprint, or no
/// longer matching the query.
#[derive(Debug)]
pub struct SprintPull {
    pub items: Vec<Pulled>,
    pub gone: Vec<String>,
}

/// Brings a sprint's view — `backlog/sprint/<slug>/` — to exactly what the
/// provider says the sprint holds, in one record on its provider ref: every
/// item in the sprint comes, and one taken out of it goes. The sprint is
/// the open one whose folder is `<slug>`, as `sprint fetch` names it.
pub fn pull_sprint(root: &Path, view: &Path, provider: &dyn Provider, config: &ProjectConfig) -> Result<SprintPull> {
    if classify(root, view) != Position::SprintView {
        bail!(msg!("pull.sprint.not_a_view", view = view.display()));
    }
    let slug = view.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let board_id = config.jira_board_id.with_context(|| msg!("config.no_board_id"))?;
    let sprint = provider
        .open_sprints(board_id)?
        .into_iter()
        .find(|s| slugify(&legible_name(s)) == slug)
        .with_context(|| msg!("pull.sprint.not_open", slug))?;
    ready_to_sync(view)?;
    let keys = provider.sprint_items(sprint.id)?;
    pull_membership(root, view, &keys, provider, config)
}

/// Brings the view of a named query — `query/<name>/` — to
/// exactly what the query returns, making the view on the first pull. The
/// query is `query` when given, for this pull only, or else the one
/// `muckpile.toml` declares under that name.
pub fn pull_query(root: &Path, view: &Path, query: Option<&str>, provider: &dyn Provider, config: &ProjectConfig) -> Result<SprintPull> {
    if classify(root, view) != Position::QueryView || view.parent() != Some(&root.join("query")) {
        bail!(msg!("pull.query.not_a_view", view = view.display()));
    }
    let name = view.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let query = match query {
        Some(query) => query.to_string(),
        None => config.queries.get(&name).cloned().with_context(|| msg!("pull.query.unknown", name))?,
    };
    if !view.exists() {
        ledger::open_view(root, &format!("query/{name}"))?;
    }
    ready_to_sync(view)?;
    let keys = provider.query_items(&query)?;
    pull_membership(root, view, &keys, provider, config)
}

/// Brings `view` to exactly the items `keys` names, in one record on its
/// provider ref: every one of them comes, and one the ref recorded that
/// isn't among them goes, with everything recorded for it.
fn pull_membership(root: &Path, view: &Path, keys: &[String], provider: &dyn Provider, config: &ProjectConfig) -> Result<SprintPull> {
    let recorded = ledger::provider_paths(view)?;
    let mut changes = Vec::new();
    let mut items = Vec::new();
    for key in keys {
        let (item_changes, pulled) = item_changes(view, key, provider, config, &recorded)?;
        changes.extend(item_changes);
        items.push(pulled);
    }
    let mut gone = Vec::new();
    for path in &recorded {
        let Some((id, _)) = path.strip_suffix(".md").and_then(|stem| stem.split_once('.')) else { continue };
        if path.contains('/') || keys.iter().any(|k| k == id) {
            continue;
        }
        gone.push(id.to_string());
        let owned = |p: &String| p == path || *p == format!(".provider/{id}.adf.json") || p.starts_with(&format!("{id}_data/"));
        changes.extend(recorded.iter().filter(|p| owned(p)).map(|p| (p.clone(), None)));
    }

    let name = view.strip_prefix(root).unwrap_or(view).to_string_lossy().into_owned();
    ledger::record(view, &changes, &format!("pull {name}"))?;
    rebase_or_explain(view)?;
    for pulled in &mut items {
        let id = pulled.path.file_name().and_then(|n| n.to_str()).and_then(|n| n.split('.').next()).unwrap_or_default().to_string();
        pulled.also_in = other_views_holding(root, view, &id)?;
    }
    Ok(SprintPull { items, gone })
}

/// The sprint view `pull` means: the one named, from the project's root —
/// `backlog/sprint/<slug>` — or, with nothing named, the one `cwd` stands
/// in. `None` otherwise: an id, or a work view, is `pull`'s own business.
pub fn sprint_view_of(root: &Path, cwd: &Path, target: Option<&str>) -> Option<PathBuf> {
    let view = match target {
        Some(target) => cwd.join(target),
        None => cwd.to_path_buf(),
    };
    let rel = view.strip_prefix(root).ok()?;
    let parts: Vec<_> = rel.components().collect();
    (parts.len() == 3 && classify(root, &view) == Position::SprintView && view.is_dir()).then_some(view)
}

/// The query view `pull` means: the one named, from the project's root —
/// `query/<name>`, which may not exist yet: the first pull makes
/// it — or, with nothing named, the one `cwd` stands in.
pub fn query_view_of(root: &Path, cwd: &Path, target: Option<&str>) -> Option<PathBuf> {
    let view = match target {
        Some(target) => cwd.join(target),
        None if cwd.is_dir() => cwd.to_path_buf(),
        None => return None,
    };
    (view.parent() == Some(&root.join("query")) && classify(root, &view) == Position::QueryView).then_some(view)
}

/// The views of the project, other than `view`, whose folder already holds
/// `<id>.<type>.md` — on this machine, right now.
fn other_views_holding(root: &Path, view: &Path, id: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for container in ["to-work", "backlog/sprint", "query"] {
        let Ok(entries) = std::fs::read_dir(root.join(container)) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path == view || !path.is_dir() {
                continue;
            }
            if find_file(&path, id).is_ok() {
                out.push(format!("{container}/{}", entry.file_name().to_string_lossy()));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Refuses a view in the middle of a rebase, or with uncommitted edits to
/// what it tracks: syncing rebases it, and setting those edits aside and
/// putting them back can clash in a way harder to follow than a rebase's
/// own. A new file nobody added yet — a fresh draft — isn't in the way, and
/// doesn't go either.
fn ready_to_sync(view: &Path) -> Result<()> {
    if ledger::rebasing(view)? {
        bail!(msg!("sync.rebasing", view = view.display()));
    }
    let changed = ledger::tracked_changes(view)?;
    if !changed.is_empty() {
        bail!(msg!("sync.uncommitted", paths = changed.join(", ")));
    }
    Ok(())
}

/// Rebases `view` onto its provider's ref, and when a clash stops it, says
/// so: settling it is a person's call, with git.
fn rebase_or_explain(view: &Path) -> Result<()> {
    match ledger::rebase(view)? {
        Rebase::Done => Ok(()),
        Rebase::Stopped(what) => bail!(msg!("sync.rebase_stopped", what)),
    }
}

/// Records on `view`'s provider ref what the provider has for `id` right
/// now, as the files of the view: the item's markdown, its ADF as the
/// provider returned it — `.provider/<id>.adf.json` —, its thread and its
/// attachments. Never touches the view itself: it's behind until it
/// rebases. The same record `pull`, a `push` that finds the provider moved,
/// and a `push` that just wrote all leave, so a view's history is always
/// what the provider said.
fn record_item(view: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig, message: &str) -> Result<Pulled> {
    let recorded = ledger::provider_paths(view)?;
    let (changes, pulled) = item_changes(view, id, provider, config, &recorded)?;
    ledger::record(view, &changes, message)?;
    Ok(pulled)
}

/// One path of a view's provider ref, set to its bytes or — `None` — gone.
type Change = (String, Option<Vec<u8>>);

/// What recording `id` would change on `view`'s provider ref — every path
/// set or gone — given what the ref already holds, and what `pull` reports
/// for it.
fn item_changes(view: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig, recorded: &[String]) -> Result<(Vec<Change>, Pulled)> {
    let item = provider.item(id)?;
    let item_type = config
        .muckpile_type_of(&item.jira_type, &item.labels)
        .with_context(|| msg!("config.no_item_type", type_name = item.jira_type))?;
    let (text, losses) = render_pulled_text(&item, provider, config)?;
    let filename = format!("{id}.{item_type}.md");

    let mut changes: Vec<Change> = Vec::new();
    // The provider may have changed the item's type since the last pull:
    // its file follows under the new name, every link to the old one
    // rewritten, in this same commit — never two files for the same item.
    for old_type in TYPES.iter().filter(|t| **t != item_type) {
        let old_name = format!("{id}.{old_type}.md");
        if !recorded.contains(&old_name) {
            continue;
        }
        changes.push((old_name.clone(), None));
        for path in recorded.iter().filter(|p| p.ends_with(".md") && **p != old_name && **p != filename) {
            let Some(other) = ledger::provider_text(view, path)? else { continue };
            let (rewritten, changed) = rewrite_type_references(&other, id, old_type, item_type);
            if changed {
                changes.push((path.clone(), Some(rewritten.into_bytes())));
            }
        }
    }
    changes.push((filename.clone(), Some(text.into_bytes())));
    changes.push((format!(".provider/{id}.adf.json"), item.body_adf.clone().map(String::into_bytes)));
    for (path, text) in thread_files(id, provider, config)? {
        changes.push((path, Some(text.into_bytes())));
    }
    for (path, bytes) in attachment_files(id, &item.attachments, provider)? {
        changes.push((path, Some(bytes)));
    }
    // A comment or an attachment the provider no longer has goes too — only
    // what its ref recorded: a draft nobody uploaded was never there.
    let brought: HashSet<String> = changes.iter().map(|(path, _)| path.clone()).collect();
    for path in recorded.iter().filter(|p| p.starts_with(&format!("{id}_data/thread/")) || p.starts_with(&format!("{id}_data/files/"))) {
        if !brought.contains(path) {
            changes.push((path.clone(), None));
        }
    }
    Ok((changes, Pulled { path: view.join(filename), losses, also_in: Vec::new() }))
}
/// Each of the item's comments as `<id>_data/thread/<comment id>.md`, and
/// its text.
fn thread_files(id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<Vec<(String, String)>> {
    provider.comments(id)?.iter().map(|comment| Ok((format!("{id}_data/thread/{}.md", comment.id), render_comment(comment, provider, config)?))).collect()
}
/// A comment as a thread file: who wrote it and when, the comment it
/// replies to, and — when it opens with `ai: <model>`, the model as code —
/// that model, taken out of the body and into the header.
fn render_comment(comment: &Comment, provider: &dyn Provider, config: &ProjectConfig) -> Result<String> {
    let markdown = to_markdown(&comment.body_adf, provider, config)?.markdown;
    let (ai, body) = split_ai(&markdown);

    let mut text = format!("---\nauthor: {}\nauthor_id: {}\ncreated: {}\n", comment.author, comment.author_id, comment.created);
    if let Some(parent) = &comment.parent {
        text.push_str(&format!("in-reply-to: {parent}\n"));
    }
    if let Some(ai) = ai {
        text.push_str(&format!("ai: {ai}\n"));
    }
    text.push_str("---\n");
    text.push_str(body);
    Ok(text)
}

/// The model a comment opens with, `ai: <model>` as its own first
/// paragraph, and the body after it — or no model, and the body whole.
fn split_ai(markdown: &str) -> (Option<&str>, &str) {
    let opened = markdown.strip_prefix("ai: `").and_then(|rest| rest.split_once("`\n"));
    match opened {
        Some((model, after)) if !model.is_empty() && !model.contains(['`', '\n']) => match after.strip_prefix('\n') {
            Some(body) => (Some(model), body),
            None if after.is_empty() => (Some(model), after),
            None => (None, markdown),
        },
        _ => (None, markdown),
    }
}

/// Each attachment as `<id>_data/files/<name>` — with its id in front when
/// another attachment shares that name — and its bytes. Only these are
/// ever recorded under `files/`: a draft nobody uploaded is the view's own.
fn attachment_files(id: &str, attachments: &[Attachment], provider: &dyn Provider) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for attachment in attachments {
        let shared = attachments.iter().filter(|a| a.filename == attachment.filename).count() > 1;
        let name = Path::new(&attachment.filename).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| attachment.id.clone());
        let name = if shared { format!("{}-{name}", attachment.id) } else { name };
        out.push((format!("{id}_data/files/{name}"), provider.attachment_content(&attachment.id)?));
    }
    Ok(out)
}

/// The exact text `pull` writes for a fetched item, and why its body can't
/// be edited locally — also what `push` uses to re-derive "what a fresh pull
/// would say right now", to compare against the last one it actually
/// committed.
fn render_pulled_text(item: &Item, provider: &dyn Provider, config: &ProjectConfig) -> Result<(String, Vec<Loss>)> {
    let mut text = String::from("---\n");
    text.push_str(&format!("title: {}\n", item.title));
    text.push_str(&format!("status: {}\n", item.status));
    if let Some(parent) = &item.parent {
        text.push_str(&format!("parent: {parent}\n"));
    }
    for (key, ids) in relations(&item.links) {
        text.push_str(&format!("relation.{key}: [{}]\n", ids.join(", ")));
    }
    text.push_str("---\n");
    let body = match &item.body_adf {
        Some(adf) => to_markdown(adf, provider, config)?,
        None => Filtered::default(),
    };
    text.push_str(&body.markdown);
    Ok((text, body.losses))
}

/// An item's links as `relation.*` fields: one key per phrase — the
/// provider's own, with `_` for each space and nothing else changed — and
/// the ids at the other end, sorted, keys sorted too.
fn relations(links: &[ItemLink]) -> BTreeMap<String, Vec<String>> {
    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for link in links {
        by_key.entry(link.phrase.replace(' ', "_")).or_default().push(link.other.clone());
    }
    for ids in by_key.values_mut() {
        ids.sort();
    }
    by_key
}

/// What catching a view up after a command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatchUp {
    /// The view doesn't hold the item: the command only wrote to the
    /// provider.
    NotHere,
    /// The item — its file and its `_data/` — was brought up to what the
    /// provider has now, and committed, as a `pull` would.
    CaughtUp,
    /// Something of the item's is uncommitted, and bringing it up would
    /// rewrite it: the view stays behind until the next `pull`. Why, to say.
    Behind(String),
}

/// After a command writes to the provider, what a `pull` of that item in
/// `view` would do — so the next `push` doesn't take the command's own write
/// for a change on the other side. Left behind, like a `pull` would refuse,
/// when the view has uncommitted edits to what it tracks or a rebase half
/// done. A draft nobody committed isn't in the way.
pub fn catch_up(view: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<CatchUp> {
    if find_file(view, id).is_err() {
        return Ok(CatchUp::NotHere);
    }
    if ledger::rebasing(view)? {
        return Ok(CatchUp::Behind(msg!("catch_up.rebasing")));
    }
    let changed = ledger::tracked_changes(view)?;
    if !changed.is_empty() {
        return Ok(CatchUp::Behind(msg!("catch_up.uncommitted", paths = changed.join(", "))));
    }
    record_item(view, id, provider, config, &format!("pull {id}"))?;
    match ledger::rebase(view)? {
        Rebase::Done => Ok(CatchUp::CaughtUp),
        Rebase::Stopped(what) => Ok(CatchUp::Behind(msg!("catch_up.rebase_stopped", what))),
    }
}
/// A body or a comment from the provider, as markdown. First, each card —
/// or ordinary link — to one of this project's items becomes a link to its
/// file, with the type the provider gives, labels included, so it doesn't
/// depend on what the view holds; then `JiraAdfMarkdownFilter`.
fn to_markdown(adf: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<Filtered> {
    let prefix = format!("{}-", config.jira_project_key);
    let key_of = |url: &str| provider.key_of_url(url).filter(|k| k.starts_with(&prefix));
    let keys: Vec<String> = cited_keys(adf, key_of)?.into_iter().collect();
    let files: BTreeMap<String, String> = if keys.is_empty() {
        BTreeMap::new()
    } else {
        provider
            .types_of(&keys)?
            .into_iter()
            .filter_map(|(key, (jira_type, labels))| config.muckpile_type_of(&jira_type, &labels).map(|t| (key.clone(), format!("{key}.{t}.md"))))
            .collect()
    };
    let translated = cards_to_file_links(adf, key_of, |key| files.get(key).cloned())?;
    JiraAdfMarkdownFilter::filter(&translated)
}

/// Markdown on its way to the provider: the converter, and then every link
/// to an item's file as a card to that item.
fn to_adf(markdown: &str, provider: &dyn Provider) -> Result<String> {
    file_links_to_cards(&body_to_adf(markdown)?, |key| provider.item_url(key))
}

/// The id of the `to-work/<id>/` view `cwd` stands exactly in, not one it's
/// merely nested under — `code-work/<repo>/` inside it is a different
/// repo's concern, not the view's own item. Shared by every command that
/// only makes sense run from inside a view's own root.
fn work_view_id(root: &Path, cwd: &Path) -> Result<String> {
    if classify(root, cwd) != Position::WorkView {
        bail!(msg!("view.not_in_work_view"));
    }
    let rel = cwd.strip_prefix(root).unwrap_or(cwd);
    let mut parts = rel.components();
    parts.next(); // "to-work"
    let id = parts.next().unwrap().as_os_str().to_string_lossy().into_owned();
    if parts.next().is_some() {
        bail!(msg!("view.not_at_root", path = rel.display()));
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
    let repo_config = config.repos.get(repo_name).with_context(|| msg!("code_work.unknown_repo", repo = repo_name))?;

    let base = root.join("base").join(repo_name);
    ensure_cloned(&repo_config.remote, &base, &repo_config.branch)?;

    let branch = match branch_override {
        Some(b) => b.to_string(),
        None => derive_branch(&id, &config.commit_prefix)?,
    };
    let from_branch = from.unwrap_or(&repo_config.branch);

    let worktree_path = cwd.join("code-work").join(repo_name);
    if worktree_path.exists() {
        bail!(msg!("path.exists", path = format!("code-work/{repo_name}")));
    }

    add_worktree(&base, &worktree_path, &branch, from_branch)?;
    Ok(worktree_path)
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
    let board_id = config.jira_board_id.with_context(|| msg!("config.no_board_id"))?;
    let sprints = provider.open_sprints(board_id)?;
    let slugs: Vec<String> = sprints.iter().map(|s| slugify(&legible_name(s))).collect();

    let dir = root.join("backlog/sprint");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut created = Vec::new();
    for slug in &slugs {
        if !dir.join(slug).exists() {
            ledger::open_view(root, &format!("backlog/sprint/{slug}"))?;
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
        let closed = if entry.path().join(".git").exists() {
            ledger::close_empty_view(root, &format!("backlog/sprint/{name}"))?
        } else if std::fs::read_dir(entry.path())?.next().is_none() {
            std::fs::remove_dir(entry.path())?;
            true
        } else {
            false
        };
        if closed {
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

/// Replaces `start`/`done`/`close`/`drop`: fires the workflow's
/// own transition leading to `target_status`, deciding by `to` — never by
/// guessing whether a transition's own name is the status it leads to. The
/// deciding logic lives in `muckpile-provider`; this only adds the id check
/// every other command already applies to an argument coming from argv,
/// and `approve` before anything is fired.
pub fn transition(id: &str, target_status: &str, provider: &dyn Provider, approve: impl FnOnce() -> Result<()>) -> Result<Outcome> {
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    approve()?;
    provider_transition(provider, id, target_status)
}

/// Replaces `depends`/`blocks` as muckpile's own vocabulary, the way
/// `transition` does for status: `phrase` is one of the provider's own —
/// the deciding-and-firing logic lives in `muckpile-provider`; this only
/// adds the id checks every other command already applies to arguments
/// coming from argv, and `approve` before anything is created.
pub fn link(a: &str, phrase: &str, b: &str, provider: &dyn Provider, approve: impl FnOnce() -> Result<()>) -> Result<LinkOutcome> {
    if !is_valid_id(a) {
        bail!(msg!("id.invalid", id = a));
    }
    if !is_valid_id(b) {
        bail!(msg!("id.invalid", id = b));
    }
    approve()?;
    provider_link(provider, a, phrase, b)
}

/// Removes, on the provider, the relation `link` with the same arguments
/// creates — the same phrase, taken the same two ways. The deciding and
/// removing lives in `muckpile-provider`; this only adds the id checks
/// every other command applies to arguments coming from argv, and `approve`
/// before anything is removed.
pub fn unlink(a: &str, phrase: &str, b: &str, provider: &dyn Provider, approve: impl FnOnce() -> Result<()>) -> Result<UnlinkOutcome> {
    if !is_valid_id(a) {
        bail!(msg!("id.invalid", id = a));
    }
    if !is_valid_id(b) {
        bail!(msg!("id.invalid", id = b));
    }
    approve()?;
    provider_unlink(provider, a, phrase, b)
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
/// body (that's canonicity's call, and `push`'s problem, not
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
    /// The header was edited by hand, and the header only changes by command:
    /// nothing of the item was sent, and the file is left as it is.
    HeaderClash(Vec<HeaderEdit>),
    /// The compare-and-swap cleared; whether the body was sent, and why not
    /// when it changed locally but wasn't safe to send.
    Written { body: bool, body_refused: Option<BodyRefused> },
    /// A pending `@slug` got a real id this run — found on the provider, or
    /// created there. `PushOutcome::id` for this case is still the original
    /// slug: the file has already moved to `id.<type>.md` by the time this
    /// comes back.
    Resolved { id: String, created: bool },
    /// A pending `@slug` couldn't be resolved this run — searching or
    /// creating it failed, or it depends on another pending item that
    /// failed first or stays local. The file is left exactly as it was,
    /// still pending.
    ResolveFailed(String),
    /// A pending `@slug` nobody committed: it stays in the view as it is,
    /// and nothing of it goes to the provider.
    Local,
    /// A relation a just-created item's draft declared didn't reach the
    /// provider. The item exists all the same; `PushOutcome::id` is its new
    /// id, and `phrase`/`other` are what `link` needs to retry it — the
    /// header is rebuilt from the provider, so the file won't carry it.
    RelationFailed { phrase: String, other: String, reason: String },
}

/// One thing in an item's header that the file says and the provider
/// doesn't — each one is what a command would change instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderEdit {
    /// `title`, `status` or `parent`: the file's value and the provider's,
    /// either absent when the field is.
    Field { name: String, written: Option<String>, provider: Option<String> },
    /// A relation the file has and the provider doesn't (`added`), or the
    /// other way round — what `link` or `unlink` does.
    Relation { phrase: String, other: String, added: bool },
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

/// What `push` is about to write to the provider, shown before it writes
/// any of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Planned {
    /// A committed `@slug`: searched by its title, and created — with its
    /// body, its parent and its relations — when nothing matches.
    Draft(String),
    /// An item whose body was edited and committed, and its header left as
    /// the provider has it: its body goes.
    Body(String),
}

/// First resolves every pending `@slug` the view has committed — search or
/// create, then rename, rewrite references, and settle a fresh baseline for
/// it, same as a `pull` — and then sends the body of every item that
/// already had a real id and changed since its own last `pull`, refusing
/// anything the provider moved on since then. The header never travels
/// through the second half: it only changes by command, so an item whose
/// header was edited by hand is reported and left exactly as it is. Before
/// any of it, `approve` sees everything that's about to go, once — and
/// nothing, and no one asked, when nothing is.
pub fn push(view: &Path, provider: &dyn Provider, config: &ProjectConfig, approve: impl FnOnce(&[Planned]) -> Result<()>) -> Result<Vec<PushOutcome>> {
    ready_to_sync(view)?;
    let planned = plan(view)?;
    if !planned.is_empty() {
        approve(&planned)?;
    }
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
        outcomes.push(push_one(view, &local, provider, config)?);
    }
    Ok(outcomes)
}

/// What `push` would write, read off the view alone: each committed draft,
/// and each item whose committed body differs from what its provider's ref
/// recorded while its header doesn't. What the provider says once asked —
/// that it moved, that a body isn't canonical — can still hold one back.
fn plan(view: &Path) -> Result<Vec<Planned>> {
    let mut planned = Vec::new();
    for pending in item::list_pending(view)? {
        if ledger::committed(view, &format!("{}.{}.md", pending.slug, pending.item_type))? {
            planned.push(Planned::Draft(pending.slug));
        }
    }
    for local in list_summaries(view)? {
        let filename = format!("{}.{}.md", local.id, local.item_type);
        let Some(recorded) = ledger::provider_text(view, &filename)? else { continue };
        let working = std::fs::read_to_string(view.join(&filename)).with_context(|| format!("reading {filename}"))?;
        let (recorded_header, recorded_body) = body::split_frontmatter(&recorded);
        let (working_header, working_body) = body::split_frontmatter(&working);
        if working_header == recorded_header && working_body != recorded_body {
            planned.push(Planned::Body(local.id));
        }
    }
    Ok(planned)
}

/// Resolves every pending `@slug` directly under `view` that's committed —
/// one nobody committed stays local, as it is, and so does what names it —
/// searching by exact title before creating, so a retry never duplicates;
/// a `parent` naming
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
    let mut local: HashSet<String> = HashSet::new();
    let mut outcomes = Vec::new();

    for slug in &order {
        let pending_item = by_slug[slug.as_str()];
        let draft = format!("{slug}.{}.md", pending_item.item_type);
        if !ledger::committed(view, &draft)? {
            local.insert(slug.clone());
            outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::Local });
            continue;
        }

        let (path, _) = find_file(view, slug)?;
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let blocked_by = read_frontmatter_refs(&text).into_iter().find(|r| by_slug.contains_key(r.as_str()) && !resolved.contains_key(r));
        if let Some(blocker) = blocked_by {
            let key = if local.contains(&blocker) { "push.resolve.blocked_local" } else { "push.resolve.blocked" };
            outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::ResolveFailed(msg!(key, blocker)) });
            continue;
        }

        let Some(provider_type) = config.item_type.get(&pending_item.item_type) else {
            outcomes.push(PushOutcome {
                id: slug.clone(),
                result: PushResult::ResolveFailed(msg!("config.no_item_type", type_name = pending_item.item_type)),
            });
            continue;
        };
        let real_id = |r: &str| resolved.get(r).cloned().unwrap_or_else(|| r.to_string());
        let real_parent = pending_item.parent.as_deref().map(real_id);

        match resolve_one(view, pending_item, provider_type, real_parent.as_deref(), provider, config) {
            Ok((id, created)) => {
                // What the draft's header declares travels only at creation,
                // like its body and its parent: a found item already existed.
                let relations = if created { read_relations(&text) } else { Vec::new() };
                let mut failed = Vec::new();
                for (phrase, others) in relations {
                    for other in others {
                        let other = real_id(&other);
                        if let Some(reason) = link_failure(provider, &id, &phrase, &other) {
                            failed.push(PushOutcome { id: id.clone(), result: PushResult::RelationFailed { phrase: phrase.clone(), other, reason } });
                        }
                    }
                }
                // What the provider made of it is recorded first, and the
                // view rebased onto it; only then is the draft retired for
                // the item's own file. The other way round, the view would
                // write that file itself and the rebase would clash on it.
                record_item(view, &id, provider, config, &format!("{} {slug}", if created { "new" } else { "found" }))?;
                rebase_or_explain(view)?;
                rename_one(view, slug, &id)?;
                resolved.insert(slug.clone(), id.clone());
                outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::Resolved { id, created } });
                outcomes.extend(failed);
            }
            Err(e) => outcomes.push(PushOutcome { id: slug.clone(), result: PushResult::ResolveFailed(e.to_string()) }),
        }
    }

    Ok(outcomes)
}

/// Creates one relation a draft declared, `id phrase other` — `None` when it
/// landed, and why it didn't otherwise.
fn link_failure(provider: &dyn Provider, id: &str, phrase: &str, other: &str) -> Option<String> {
    match provider_link(provider, id, phrase, other) {
        Ok(LinkOutcome::Applied { .. }) => None,
        Ok(LinkOutcome::NoSuchPhrase { .. }) => Some(msg!("push.relation.no_such_phrase", phrase)),
        Err(e) => Some(e.to_string()),
    }
}

/// One pending item, already past the batch-dependency check: search by
/// title — and by label, when its type needs one to be told apart — and only
/// create when nothing matches. A found item never sends its draft body or
/// parent — both only ever travel at creation, and finding means something
/// already existed before this run touched it.
fn resolve_one(
    view: &Path,
    pending: &item::PendingItem,
    provider_type: &ItemType,
    real_parent: Option<&str>,
    provider: &dyn Provider,
    config: &ProjectConfig,
) -> Result<(String, bool)> {
    let (jira_type, label) = (provider_type.jira_type.as_str(), provider_type.label.as_deref());
    if let Some(id) = provider.find_by_title(&config.jira_project_key, jira_type, label, &pending.title)? {
        return Ok((id, false));
    }
    let draft = format!("{}.{}.md", pending.slug, pending.item_type);
    let body = settle_canonical(view, &draft, &pending.body, provider, config)?;
    let body_adf = if body.trim().is_empty() { None } else { Some(to_adf(&body, provider)?) };
    let id = provider.create_item(&config.jira_project_key, jira_type, label, &pending.title, real_parent, body_adf.as_deref())?;
    Ok((id, true))
}

/// The body that goes out, in the form it comes back in: when converting it
/// to the provider's format and back changes it — a bold cut around a code
/// span — the file is rewritten to that form first, in a commit of its own
/// signed as the tool, so what was changed to send it is there to see in
/// git, and what comes back matches what the view has.
fn settle_canonical(view: &Path, filename: &str, body: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<String> {
    if body.trim().is_empty() {
        return Ok(body.to_string());
    }
    let canonical = to_markdown(&to_adf(body, provider)?, provider, config)?.markdown;
    if canonical == body {
        return Ok(canonical);
    }
    let path = view.join(filename);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let (frontmatter, _) = body::split_frontmatter(&text);
    std::fs::write(&path, format!("{frontmatter}{canonical}")).with_context(|| format!("writing {}", path.display()))?;
    commit_paths(view, &[filename], &format!("forma canónica de {}", filename.trim_end_matches(".md")))?;
    Ok(canonical)
}
fn push_one(view: &Path, local: &ItemSummary, provider: &dyn Provider, config: &ProjectConfig) -> Result<PushOutcome> {
    let filename = format!("{}.{}.md", local.id, local.item_type);
    let outcome = |result| PushOutcome { id: local.id.clone(), result };

    let Some(recorded) = ledger::provider_text(view, &filename)? else {
        return Ok(outcome(PushResult::NeverPulled));
    };
    let working_path = view.join(&filename);
    let working = std::fs::read_to_string(&working_path).with_context(|| format!("reading {}", working_path.display()))?;
    if working == recorded {
        return Ok(outcome(PushResult::Unchanged));
    }

    // Ask again before writing: what the provider has right now has to still
    // be what its ref last recorded — the ADF against the ADF, which sees
    // what the markdown can't, like a table's rows numbered after the pull.
    // If it moved, nothing is written: what it has is recorded, and the view
    // rebased onto it, for a person to look at before pushing again.
    let remote = provider.item(&local.id)?;
    let (remote_text, losses) = render_pulled_text(&remote, provider, config)?;
    let recorded_adf = ledger::provider_text(view, &format!(".provider/{}.adf.json", local.id))?;
    if remote_text != recorded || !body::same_adf(remote.body_adf.as_deref(), recorded_adf.as_deref())? {
        record_item(view, &local.id, provider, config, &format!("pull {}", local.id))?;
        rebase_or_explain(view)?;
        return Ok(outcome(PushResult::Stale));
    }

    // The header only changes by command. Nothing of the item goes — not
    // even a body edit next to it: what records the send comes from the
    // provider, and would take the header edit with it.
    let edits = header_edits(&local.id, &local.item_type, &working, &remote)?;
    if !edits.is_empty() {
        return Ok(outcome(PushResult::HeaderClash(edits)));
    }

    let (_, recorded_body) = body::split_frontmatter(&recorded);
    let (_, working_body) = body::split_frontmatter(&working);
    let mut body_sent = false;
    let mut body_refused = None;
    if working_body != recorded_body {
        // Canonicity is decided on the provider's ADF as it is right now —
        // just confirmed to still be what its ref recorded. Only a body read
        // from some ADF can have losses.
        if let Some(real) = remote.body_adf.as_deref().filter(|_| !losses.is_empty()) {
            body_refused = Some(BodyRefused { losses, diff: body::adf_diff(real, &to_adf(working_body, provider)?)? });
        } else {
            let body = settle_canonical(view, &filename, working_body, provider, config)?;
            provider.update_body(&local.id, &to_adf(&body, provider)?)?;
            body_sent = true;
            // What the provider has after the write is recorded, and the
            // view rebased onto it: the view's own commit of the edit goes
            // away, because the provider's ref now holds the same change.
            record_item(view, &local.id, provider, config, &format!("push {}", local.id))?;
            rebase_or_explain(view)?;
        }
    }

    Ok(outcome(PushResult::Written { body: body_sent, body_refused }))
}

/// Every header field `working` says differently from the provider — the
/// fields one by one, then each relation added or taken out, ordered.
fn header_edits(id: &str, item_type: &str, working: &str, remote: &Item) -> Result<Vec<HeaderEdit>> {
    let written = parse_full(id.to_string(), item_type.to_string(), working)?;
    let mut edits = Vec::new();
    let mut compare = |name: &str, written: Option<String>, provider: Option<String>| {
        if written != provider {
            edits.push(HeaderEdit::Field { name: name.to_string(), written, provider });
        }
    };
    compare("title", Some(written.title), Some(remote.title.clone()));
    compare("status", written.status, Some(remote.status.clone()));
    compare("parent", written.parent, remote.parent.clone());

    let pairs = |relations: Vec<(String, Vec<String>)>| -> BTreeSet<(String, String)> {
        relations.into_iter().flat_map(|(phrase, others)| others.into_iter().map(move |o| (phrase.clone(), o))).collect()
    };
    let in_file = pairs(read_relations(working));
    let on_provider = pairs(relations(&remote.links).into_iter().collect());
    let mut changed: Vec<(&(String, String), bool)> =
        in_file.difference(&on_provider).map(|pair| (pair, true)).chain(on_provider.difference(&in_file).map(|pair| (pair, false))).collect();
    changed.sort();
    edits.extend(changed.into_iter().map(|((phrase, other), added)| HeaderEdit::Relation { phrase: phrase.clone(), other: other.clone(), added }));
    Ok(edits)
}


/// The only way an item's title changes: written to the provider right
/// away once `approve` lets it, with no conversion — `push` never sends a
/// title edited by hand.
pub fn title(id: &str, new_title: &str, provider: &dyn Provider, approve: impl FnOnce() -> Result<()>) -> Result<()> {
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    if new_title.trim().is_empty() {
        bail!(msg!("title.empty"));
    }
    approve()?;
    provider.update_title(id, new_title)
}

/// The only way the parent of an already-synced item changes: written to the
/// provider right away — `push` never sends a `parent` edited by hand. That
/// the parent exists, and can hold this item, is the provider's to say;
/// that it goes at all, `approve`'s.
pub fn parent(id: &str, parent_id: &str, provider: &dyn Provider, approve: impl FnOnce() -> Result<()>) -> Result<()> {
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    if !is_valid_id(parent_id) {
        bail!(msg!("id.invalid", id = parent_id));
    }
    if id == parent_id {
        bail!(msg!("parent.self", id));
    }
    approve()?;
    provider.set_parent(id, parent_id)
}

/// Who wrote a comment — every comment says, one way or the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Author<'a> {
    /// A model, named: it goes at the top of the comment, where anyone
    /// reading the provider sees it and `pull` reads it back.
    Ai(&'a str),
    /// A person, confirmed at a terminal: nothing is added to the comment.
    Human(HumanProof),
}

/// That someone at a terminal retyped a phrase they were just shown — the
/// only way to build an `Author::Human`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HumanProof(());

/// Shows `ask` a phrase that's different every time, and confirms only when
/// what comes back is that phrase. `ask` is the terminal itself — never
/// standard input, which a pipe can fill — so with no terminal there's no
/// answer, and no confirmation.
pub fn confirm_human(ask: impl FnOnce(&str) -> Result<String>) -> Result<HumanProof> {
    confirm(ask, "comment.human.no_terminal", "comment.human.mismatch")
}

/// Stands before every command that creates or edits on the provider: lets
/// it through when the project writes on its own — `auto_update` — and
/// otherwise only when a person retypes the phrase `ask` shows them, the
/// same way `confirm_human` does. Never asks when it doesn't have to.
pub fn approve_update(config: &ProjectConfig, ask: impl FnOnce(&str) -> Result<String>) -> Result<()> {
    if !config.auto_update {
        confirm(ask, "update.human.no_terminal", "update.human.mismatch")?;
    }
    Ok(())
}

fn confirm(ask: impl FnOnce(&str) -> Result<String>, no_terminal: &str, mismatch: &str) -> Result<HumanProof> {
    let phrase = random_phrase();
    let typed = ask(&phrase).with_context(|| msg!(no_terminal))?;
    if typed.trim() != phrase {
        bail!(msg!(mismatch));
    }
    Ok(HumanProof(()))
}

/// Two short words, `noun-adjective` — `faro-azul`, `puma-veloz` — with no
/// accents and adjectives that don't change with gender, so it's quick to
/// type and always reads right.
pub fn random_phrase() -> String {
    const NOUNS: [&str; 40] = [
        "faro", "nube", "puma", "lago", "sol", "luna", "mar", "pino", "roble", "tren", "barco", "puente", "cerro", "valle",
        "bosque", "llama", "condor", "zorro", "lobo", "oso", "gato", "tigre", "delfin", "halcon", "cometa", "volcan", "isla",
        "selva", "playa", "piedra", "arena", "trueno", "rayo", "viento", "fuego", "hielo", "cielo", "puerto", "ancla", "mapa",
    ];
    const ADJECTIVES: [&str; 36] = [
        "azul", "verde", "gris", "feliz", "veloz", "libre", "leal", "fiel", "breve", "suave", "fuerte", "grande", "dulce",
        "audaz", "capaz", "tenaz", "joven", "alegre", "triste", "noble", "firme", "simple", "sutil", "feroz", "sagaz", "voraz",
        "fugaz", "lunar", "solar", "polar", "astral", "rural", "real", "total", "genial", "gentil",
    ];
    format!("{}-{}", NOUNS[random_below(NOUNS.len())], ADJECTIVES[random_below(ADJECTIVES.len())])
}

/// A number below `n`, from the random keys the standard library already
/// seeds its hash maps with — enough for a phrase that only has to differ.
fn random_below(n: usize) -> usize {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default());
    (hasher.finish() % n as u64) as usize
}

/// Sends the markdown in `file` as a comment on `id` — a reply to
/// `reply_to` when given — and returns the new comment's id. Refused unless
/// it says who wrote it, so a comment with no model on top is never one
/// that forgot to say; and refused from a model unless the project lets one
/// comment — `auto_comment`.
pub fn comment(id: &str, file: &Path, reply_to: Option<&str>, author: Option<Author>, provider: &dyn Provider, config: &ProjectConfig) -> Result<String> {
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    if let Some(parent) = reply_to {
        if parent.is_empty() || !parent.chars().all(|c| c.is_ascii_digit()) {
            bail!(msg!("comment.reply_to.invalid", id = parent));
        }
    }
    let Some(author) = author else {
        bail!(msg!("comment.no_author"));
    };
    if matches!(author, Author::Ai(_)) && !config.auto_comment {
        bail!(msg!("comment.ai.not_allowed"));
    }
    let text = std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let markdown = match author {
        Author::Ai(model) => {
            if model.trim().is_empty() || model.contains(['`', '\n']) {
                bail!(msg!("comment.ai.bad_model", model = format!("{model:?}")));
            }
            format!("ai: `{model}`\n\n{text}")
        }
        Author::Human(_) => text,
    };
    provider.add_comment(id, &to_adf(&markdown, provider)?, reply_to)
}

/// Uploads `file` as an attachment of `id`, under its own name, once
/// `approve` lets it.
pub fn attach(id: &str, file: &Path, provider: &dyn Provider, approve: impl FnOnce() -> Result<()>) -> Result<Attachment> {
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    let filename = file.file_name().map(|n| n.to_string_lossy().into_owned()).with_context(|| msg!("attach.not_a_file", path = file.display()))?;
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    approve()?;
    provider.add_attachment(id, &filename, &bytes)
}

/// What `show` prints — frontmatter plus body, live or local, and the
/// `<id>_data/` listing alongside either, since that directory
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
/// ADF the same way `pull` does — cards to items as links to their files
/// included.
pub fn show_live(dir: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<Show> {
    if !is_valid_id(id) {
        bail!(msg!("id.invalid", id));
    }
    let item = provider.item(id)?;
    let body = match &item.body_adf {
        Some(adf) => to_markdown(adf, provider, config)?.markdown,
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
/// directory — `thread` and `files` once `pull` has written a comment or an
/// attachment there, and anything a person put there alongside them.
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
