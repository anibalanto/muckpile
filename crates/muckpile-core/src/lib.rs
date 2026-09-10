pub mod body;
pub mod codework;
pub mod identity;
pub mod item;
pub mod project;
pub mod states;

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The item types a view can hold. `sprint` isn't here: it names a folder
/// under `backlog/sprint/`, never a `<id>.<type>.md` file.
pub const TYPES: [&str; 4] = ["task", "user-story", "epic", "question"];

/// The marker of what exists only on this side, not assigned a provider key
/// yet. Declared, not inferred from key shape — a bare integer id is a valid
/// provider key for some providers, so there's no format to infer it from.
pub const MARKER: char = '@';

/// A valid id: the marker, optionally, then `[A-Za-z0-9_-]+`. `.` is left out
/// because it separates the type, `/` because items live flat in a view.
pub fn is_valid_id(id: &str) -> bool {
    let body = id.strip_prefix(MARKER).unwrap_or(id);
    !body.is_empty() && body.chars().all(is_id_char)
}

pub fn is_unassigned(slug: &str) -> bool {
    slug.starts_with(MARKER)
}

/// The mechanical rule `worklist new` already documents: lowercase, common
/// Latin accents stripped, and any run of characters outside `[a-z0-9]`
/// collapsed to one `-`, trimmed from both ends. No length cap — a title is
/// as long as it needs to be, and cutting it short would stop answering the
/// question a slug exists for.
pub fn slugify_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut pending_hyphen = false;
    for c in title.chars() {
        let base = strip_latin_accent(c).to_ascii_lowercase();
        if base.is_ascii_lowercase() || base.is_ascii_digit() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.push(base);
        } else {
            pending_hyphen = true;
        }
    }
    out
}

fn strip_latin_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        _ => c,
    }
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// What is **not** an id character, written once: the marker counts as part
/// of the id, so `@a` doesn't match inside `x@a`.
const NOT_ID_CHAR: &str = "[^@A-Za-z0-9_-]";

fn boundary(pattern: &str) -> Regex {
    Regex::new(&format!("{pattern}(?:{NOT_ID_CHAR}|$)")).unwrap()
}

/// Finds `<slug>.<type>.md` in `view`, trying every known type.
pub fn find_file(view: &Path, slug: &str) -> Result<(PathBuf, String)> {
    for t in TYPES {
        let p = view.join(format!("{slug}.{t}.md"));
        if p.exists() {
            return Ok((p, t.to_string()));
        }
    }
    Err(anyhow!("no {slug}.<type>.md in {}", view.display()))
}

/// Rewrites, in `text`, every delimited reference to `old_slug` (link,
/// backtick, frontmatter field) into `new_id`. Never touches a bare
/// substring: `old_slug` surrounded by id characters on either side doesn't
/// match.
pub fn rewrite_references(text: &str, old_slug: &str, old_type: &str, new_id: &str) -> (String, bool) {
    let mut changed = false;
    let mut out = text.to_string();

    // 1. link destinations: ](old_slug.type.md — with the `../` that may
    // lead it, since a view can nest a sprint view under a reference.
    let link_pat = format!(r"\]\((?:\.\./)*{}\.{}\.md", regex::escape(old_slug), regex::escape(old_type));
    let link_re = boundary(&link_pat);
    out = replace_link(&link_re, &out, &mut changed, new_id, old_type);

    // 2. frontmatter: `parent: <slug>` and any `relation.<name>:` list or
    // bare value — the name is the provider's own phrase with `_` for each
    // space, so it can carry a capital or an accent: anything but a space or
    // the colon that ends it.
    if let Some(fm_end) = frontmatter_end(&out) {
        let (fm, rest) = out.split_at(fm_end);
        let mut fm = fm.to_string();

        // The delimiter is captured and restored: losing it glues the next
        // YAML line when `parent:` is the frontmatter's last line, and the
        // closing `---` stops separating — the whole file becomes body.
        let parent_re =
            Regex::new(&format!(r"parent:\s*{}({NOT_ID_CHAR}|$)", regex::escape(old_slug))).unwrap();
        if parent_re.is_match(&fm) {
            fm = parent_re.replace(&fm, format!("parent: {new_id}${{1}}")).to_string();
            changed = true;
        }

        let rel_re = Regex::new(r"(relation\.[^\s:]+:\s*)(\[[^\]]*\]|[^\n]+)").unwrap();
        let slug_re = boundary(&regex::escape(old_slug));
        fm = rel_re
            .replace_all(&fm, |caps: &regex::Captures| {
                let prefix = &caps[1];
                let body = &caps[2];
                let new_body = replace_boundary_word(&slug_re, body, &mut changed, new_id);
                format!("{prefix}{new_body}")
            })
            .to_string();

        out = format!("{fm}{rest}");
    }

    // 3. ids between backticks in prose: `old_slug`
    let backtick_pat = format!("`{}`", regex::escape(old_slug));
    let backtick_re = boundary(&backtick_pat);
    out = replace_boundary(&backtick_re, &out, &mut changed, &format!("`{new_id}`"));

    (out, changed)
}

/// Rewrites, in `text`, every link to `<id>.<old_type>.md` into
/// `<id>.<new_type>.md`, keeping the `../` that may lead it. Only links:
/// `parent`, `relation.*` and ids in prose name the id, which a type change
/// keeps.
pub fn rewrite_type_references(text: &str, id: &str, old_type: &str, new_type: &str) -> (String, bool) {
    let mut changed = false;
    let link_re = boundary(&format!(r"\]\((?:\.\./)*{}\.{}\.md", regex::escape(id), regex::escape(old_type)));
    let out = replace_link(&link_re, text, &mut changed, id, new_type);
    (out, changed)
}

/// Moves `<id>.<old_type>.md` to `<id>.<new_type>.md` in `view` and rewrites
/// every link to the old name — without committing: the caller commits it
/// together with whatever it's recording. Returns every path touched,
/// relative to `view`, the old name and the new one included.
pub fn retype(view: &Path, id: &str, old_type: &str, new_type: &str) -> Result<Vec<String>> {
    let old_name = format!("{id}.{old_type}.md");
    let new_name = format!("{id}.{new_type}.md");
    let mut touched = Vec::new();
    for path in markdown_files(view) {
        let text = std::fs::read_to_string(&path)?;
        let (new_text, changed) = rewrite_type_references(&text, id, old_type, new_type);
        if changed {
            std::fs::write(&path, new_text)?;
            touched.push(path.strip_prefix(view).unwrap_or(&path).to_string_lossy().to_string());
        }
    }
    std::fs::rename(view.join(&old_name), view.join(&new_name)).with_context(|| format!("renaming {old_name} to {new_name}"))?;
    touched.push(old_name);
    touched.push(new_name);
    Ok(touched)
}

fn replace_boundary(re: &Regex, text: &str, changed: &mut bool, new_head: &str) -> String {
    replace_boundary_inner(re, text, changed, new_head, false)
}

/// Like `replace_boundary`, but also requiring a **left** boundary — for the
/// pattern that starts with the raw slug, where no literal delimiter guards
/// it up front. Without this, renaming `j` reaches inside `2j`.
fn replace_boundary_word(re: &Regex, text: &str, changed: &mut bool, new_head: &str) -> String {
    replace_boundary_inner(re, text, changed, new_head, true)
}

fn replace_boundary_inner(re: &Regex, text: &str, changed: &mut bool, new_head: &str, check_left: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        if check_left && !left_is_boundary(text, m.start()) {
            continue;
        }
        let matched = m.as_str();
        let head_len = matched.len() - trailing_delim_len(matched);
        out.push_str(&text[last..m.start()]);
        out.push_str(new_head);
        out.push_str(&matched[head_len..]);
        last = m.end();
        *changed = true;
    }
    out.push_str(&text[last..]);
    out
}

fn left_is_boundary(text: &str, at: usize) -> bool {
    match text[..at].chars().next_back() {
        None => true,
        Some(c) => !is_id_char(c) && c != MARKER,
    }
}

fn trailing_delim_len(matched: &str) -> usize {
    match matched.chars().last() {
        Some(c) if !is_id_char(c) && c != MARKER => c.len_utf8(),
        _ => 0,
    }
}

fn frontmatter_end(text: &str) -> Option<usize> {
    if !text.starts_with("---\n") {
        return None;
    }
    let rest = &text[4..];
    let idx = rest.find("\n---\n")?;
    Some(4 + idx + 5)
}

/// The ids (`parent` plus every `relation.*`) that `text`'s frontmatter names.
pub fn read_frontmatter_refs(text: &str) -> HashSet<String> {
    let mut refs = HashSet::new();
    let Some(end) = frontmatter_end(text) else { return refs };
    let fm = &text[..end];

    let parent_re = Regex::new(r"(?m)^parent:\s*(\S+)").unwrap();
    if let Some(c) = parent_re.captures(fm) {
        refs.insert(c[1].to_string());
    }
    for (_, ids) in read_relations(text) {
        refs.extend(ids);
    }
    refs
}

/// Every `relation.<key>:` in `text`'s frontmatter, in the order written:
/// the key as it stands, and the ids it lists — a bracketed list or a bare
/// value.
pub fn read_relations(text: &str) -> Vec<(String, Vec<String>)> {
    let Some(end) = frontmatter_end(text) else { return Vec::new() };
    let rel_re = Regex::new(r"(?m)^relation\.([^\s:]+):\s*(\[[^\]]*\]|\S+)").unwrap();
    let id_re = Regex::new(r"@?[A-Za-z0-9_-]+").unwrap();
    rel_re
        .captures_iter(&text[..end])
        .map(|c| (c[1].to_string(), id_re.find_iter(&c[2]).map(|m| m.as_str().to_string()).collect()))
        .collect()
}

/// Topological order of `slugs` (all unassigned) by their references to other
/// slugs in the same batch. Errors on a cycle.
pub fn topo_order(view: &Path, slugs: &[String]) -> Result<Vec<String>> {
    let slug_set: HashSet<&str> = slugs.iter().map(|s| s.as_str()).collect();
    let mut deps: HashMap<String, HashSet<String>> = HashMap::new();
    for slug in slugs {
        let (path, _) = find_file(view, slug)?;
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let refs = read_frontmatter_refs(&text);
        let filtered: HashSet<String> = refs.into_iter().filter(|r| slug_set.contains(r.as_str())).collect();
        deps.insert(slug.clone(), filtered);
    }

    let mut order = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();

    fn visit(
        s: &str,
        deps: &HashMap<String, HashSet<String>>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(s) {
            return Ok(());
        }
        if visiting.contains(s) {
            bail!("cycle detected at {s}");
        }
        visiting.insert(s.to_string());
        for dep in &deps[s] {
            visit(dep, deps, visiting, visited, order)?;
        }
        visiting.remove(s);
        visited.insert(s.to_string());
        order.push(s.to_string());
        Ok(())
    }

    for s in slugs {
        visit(s, &deps, &mut visiting, &mut visited, &mut order)?;
    }
    Ok(order)
}

fn git(view: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(view)
        .args(args)
        .status()
        .with_context(|| format!("running git {:?}", args))?;
    if !status.success() {
        bail!("git {:?} failed with {status}", args);
    }
    Ok(())
}

/// Commits exactly `paths` (relative to `view`) into whatever git repository
/// governs `view`, if any of them actually differ from what's already
/// committed — a no-op, returning `false`, otherwise. `git add -A` with no
/// pathspec stages the *whole* repository, not just `view`'s subtree
/// (measured): naming `paths` explicitly is what keeps this from sweeping in
/// another view's unrelated, still-uncommitted edit.
pub fn commit_paths(view: &Path, paths: &[&str], message: &str) -> Result<bool> {
    if paths.is_empty() || status_of(view, paths)?.trim().is_empty() {
        return Ok(false);
    }

    let mut add_args = vec!["add", "--"];
    add_args.extend(paths);
    git(view, &add_args)?;
    git(view, &["commit", "-q", "-m", message])?;
    Ok(true)
}

fn status_of(view: &Path, paths: &[&str]) -> Result<String> {
    let mut args = vec!["status", "--porcelain", "--"];
    args.extend(paths);
    let out = Command::new("git")
        .arg("-C")
        .arg(view)
        .args(&args)
        .output()
        .with_context(|| format!("running git {:?}", args))?;
    if !out.status.success() {
        bail!("git {:?} failed with {}", args, out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The content `path` (relative to `view`) had at the tip of whatever git
/// repository governs `view` — `None` when there's no commit yet, or none
/// that ever carried this path. The caller can't tell those two apart from
/// this alone, and doesn't need to: both mean "nothing to compare against".
pub fn head_text(view: &Path, path: &str) -> Result<Option<String>> {
    // `HEAD:<path>` resolves from the repository root, not from `-C`'s
    // directory — measured. The leading `./` is what makes it relative to
    // `view`, the same way `git show HEAD:./file` already means "the file
    // named `file` right here".
    let out = Command::new("git")
        .arg("-C")
        .arg(view)
        .args(["show", &format!("HEAD:./{path}")])
        .output()
        .with_context(|| format!("running git show HEAD:./{path}"))?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Every file a reference can be written in, under `view`: the `.md` items,
/// recursively — a view can nest a sprint's worth of them — but never
/// `code-work/`, which is a worktree of the external project's own repo and
/// not muckpile's to rewrite.
fn markdown_files(view: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![view.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            let name = name.to_string_lossy();
            if p.is_dir() {
                if name != ".git" && name != "code-work" {
                    stack.push(p);
                }
            } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Like `replace_boundary`, but keeping the `../` the match captured: what
/// changes is the file's name, not where it lives.
fn replace_link(re: &Regex, text: &str, changed: &mut bool, new_id: &str, item_type: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        let matched = m.as_str();
        let dots = matched.strip_prefix("](").map(|r| r.len() - r.trim_start_matches("../").len()).unwrap_or(0);
        let prefix = &matched[2..2 + dots];
        let head_len = matched.len() - trailing_delim_len(matched);
        out.push_str(&text[last..m.start()]);
        out.push_str(&format!("]({prefix}{new_id}.{item_type}.md"));
        out.push_str(&matched[head_len..]);
        last = m.end();
        *changed = true;
    }
    out.push_str(&text[last..]);
    out
}

/// Renames an item, its sibling `_data/` directory if it has one, and
/// rewrites every reference across the view, in one commit. Returns the
/// files touched, not counting the renamed item itself.
pub fn rename_one(view: &Path, old_slug: &str, new_id: &str) -> Result<Vec<String>> {
    let (src, item_type) = find_file(view, old_slug)?;
    let dst_name = format!("{new_id}.{item_type}.md");

    let mut touched = Vec::new();
    for path in markdown_files(view) {
        let text = std::fs::read_to_string(&path)?;
        let (new_text, changed) = rewrite_references(&text, old_slug, &item_type, new_id);
        if changed {
            std::fs::write(&path, new_text)?;
            touched.push(path.strip_prefix(view).unwrap_or(&path).to_string_lossy().to_string());
        }
    }

    git(view, &["mv", src.file_name().unwrap().to_str().unwrap(), &dst_name])?;

    let old_data = format!("{old_slug}_data");
    if view.join(&old_data).is_dir() {
        git(view, &["mv", &old_data, &format!("{new_id}_data")])?;
    }

    git(view, &["add", "-A"])?;
    let msg = if touched.is_empty() {
        format!("rename {old_slug} -> {new_id}")
    } else {
        format!("rename {old_slug} -> {new_id} ({} refs)", touched.len())
    };
    git(view, &["commit", "-q", "-m", &msg])?;
    Ok(touched)
}

/// Renames a batch of requests in topological order. On a cycle, nothing is
/// written: `topo_order` fails before touching the view.
pub fn resolve_batch(view: &Path, slug_to_id: &HashMap<String, String>) -> Result<()> {
    let slugs: Vec<String> = slug_to_id.keys().cloned().collect();
    let order = topo_order(view, &slugs)?;
    for slug in order {
        let new_id = &slug_to_id[&slug];
        rename_one(view, &slug, new_id)?;
    }
    Ok(())
}
