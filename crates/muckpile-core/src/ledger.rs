//! The ledger: a project's own git, `.muckpile/`, holding what the provider
//! said and what was done on top of it. It has no worktree of its own: each
//! view is a worktree of it, on its own branch, and each view has a second
//! ref, `refs/remotes/provider/<view>`, the view branch's upstream. That ref
//! only ever moves forward, and only with what the provider returned; the
//! view is rebased on top of it, so a view's history is always the
//! provider's, with the view's own commits on top.

use anyhow::{anyhow, bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The ledger's directory, at a project's root.
pub const LEDGER: &str = ".muckpile";

/// The name the tool commits under — a person's commits keep their own.
const TOOL_NAME: &str = "muckpile";
const TOOL_EMAIL: &str = "muckpile@localhost";

/// Where a rebase ended up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rebase {
    Done,
    /// It stopped on a clash, for a person to settle with git: what git
    /// said about it.
    Stopped(String),
}

/// Creates `<project>/.muckpile/`, a git with no worktree of its own.
pub fn init(project: &Path) -> Result<()> {
    let ledger = project.join(LEDGER);
    if ledger.exists() {
        bail!("{}: ya existe", ledger.display());
    }
    run(project, &["init", "-q", "--bare", LEDGER])?;
    Ok(())
}

/// Opens a view at `<project>/<name>` — `to-work/SGE-9876`,
/// `backlog/sprint/22_Las_vistas` — as a worktree of the ledger on a branch
/// of that same name. The branch and its provider's ref both start at one
/// commit with no files, and the provider's ref is the branch's upstream.
pub fn open_view(project: &Path, name: &str) -> Result<PathBuf> {
    let ledger = project.join(LEDGER);
    let view = project.join(name);
    if view.exists() {
        bail!("{name}: ya existe");
    }
    let empty_tree = git_in(&ledger, &["hash-object", "-t", "tree", "-w", "--stdin"], Some(b""))?;
    let root = tool_git_in(&ledger, &["commit-tree", &empty_tree, "-m", &format!("vista {name}")], None)?;
    git_in(&ledger, &["update-ref", &provider_ref_of(name), &root], None)?;
    git_in(&ledger, &["branch", name, &root], None)?;
    git_in(&ledger, &["config", &format!("branch.{name}.remote"), "."], None)?;
    git_in(&ledger, &["config", &format!("branch.{name}.merge"), &provider_ref_of(name)], None)?;
    if let Some(parent) = view.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    git_in(&ledger, &["worktree", "add", "-q", &view.to_string_lossy(), name], None)?;
    Ok(view)
}

/// The branch `view` stands on — the view's own name.
pub fn branch(view: &Path) -> Result<String> {
    git_in(view, &["symbolic-ref", "--short", "HEAD"], None)
}

/// The provider's ref of `view`.
pub fn provider_ref(view: &Path) -> Result<String> {
    Ok(provider_ref_of(&branch(view)?))
}

fn provider_ref_of(name: &str) -> String {
    format!("refs/remotes/provider/{name}")
}

/// The bytes of `path` (relative to the view's root) as the provider's ref
/// holds it — `None` when it doesn't.
pub fn provider_file(view: &Path, path: &str) -> Result<Option<Vec<u8>>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(view)
        .args(["show", &format!("{}:{path}", provider_ref(view)?)])
        .output()
        .context("running git show")?;
    Ok(out.status.success().then_some(out.stdout))
}

/// Like `provider_file`, as text.
pub fn provider_text(view: &Path, path: &str) -> Result<Option<String>> {
    Ok(provider_file(view, path)?.map(|b| String::from_utf8_lossy(&b).into_owned()))
}

/// Every path the provider's ref holds, relative to the view's root.
pub fn provider_paths(view: &Path) -> Result<Vec<String>> {
    let listing = git_in(view, &["ls-tree", "-r", "--name-only", &provider_ref(view)?], None)?;
    Ok(listing.lines().map(str::to_string).collect())
}

/// Records, as one commit on `view`'s provider ref, what the provider has
/// now: each path set to its bytes, or — `None` — gone. Never touches the
/// view itself: it's behind until it rebases. Returns `false`, and makes no
/// commit, when the provider's ref already holds exactly that.
pub fn record(view: &Path, changes: &[(String, Option<Vec<u8>>)], message: &str) -> Result<bool> {
    let reference = provider_ref(view)?;
    let parent = git_in(view, &["rev-parse", &reference], None)?;
    let index = PathBuf::from(git_in(view, &["rev-parse", "--path-format=absolute", "--git-path", "muckpile-record-index"], None)?);
    let result = (|| -> Result<bool> {
        let with_index = |args: &[&str], stdin: Option<&[u8]>| git_env(view, args, stdin, &[("GIT_INDEX_FILE", index.as_os_str())], false);
        with_index(&["read-tree", &reference], None)?;
        for (path, bytes) in changes {
            match bytes {
                Some(bytes) => {
                    let blob = with_index(&["hash-object", "-w", "--stdin"], Some(bytes))?;
                    with_index(&["update-index", "--add", "--cacheinfo", &format!("100644,{blob},{path}")], None)?;
                }
                None => {
                    with_index(&["update-index", "--force-remove", path], None)?;
                }
            }
        }
        let tree = with_index(&["write-tree"], None)?;
        if tree == git_in(view, &["rev-parse", &format!("{reference}^{{tree}}")], None)? {
            return Ok(false);
        }
        let commit = tool_git_in(view, &["commit-tree", &tree, "-p", &parent, "-m", message], None)?;
        git_in(view, &["update-ref", &reference, &commit, &parent], None)?;
        Ok(true)
    })();
    let _ = std::fs::remove_file(&index);
    result
}

/// Rebases `view` onto its provider's ref: the view's own commits are
/// replayed on top of what the provider has now, and one whose change the
/// provider already has goes away. A clash stops it, for a person to settle.
pub fn rebase(view: &Path) -> Result<Rebase> {
    let reference = provider_ref(view)?;
    let out = Command::new("git")
        .arg("-C")
        .arg(view)
        .args(["-c", &format!("user.name={TOOL_NAME}"), "-c", &format!("user.email={TOOL_EMAIL}"), "rebase", "-q", &reference])
        .output()
        .context("running git rebase")?;
    if out.status.success() {
        return Ok(Rebase::Done);
    }
    if rebasing(view)? {
        return Ok(Rebase::Stopped(format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_string()));
    }
    bail!("git rebase {reference} failed: {}", String::from_utf8_lossy(&out.stderr).trim())
}

/// Whether `view` is in the middle of a rebase someone has to finish.
pub fn rebasing(view: &Path) -> Result<bool> {
    for state in ["rebase-merge", "rebase-apply"] {
        let path = git_in(view, &["rev-parse", "--path-format=absolute", "--git-path", state], None)?;
        if Path::new(&path).exists() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The tracked files `view` has uncommitted changes to — staged or not. A
/// file git doesn't track yet, like a fresh draft, isn't one.
pub fn tracked_changes(view: &Path) -> Result<Vec<String>> {
    let status = git_in(view, &["status", "--porcelain", "--untracked-files=no"], None)?;
    Ok(status.lines().map(|line| line.get(3..).unwrap_or(line).to_string()).collect())
}

/// Commits, signed as the tool, exactly `paths` of `view` as they are on
/// disk — added, changed, or gone.
pub fn tool_commit(view: &Path, paths: &[&str], message: &str) -> Result<()> {
    let mut add = vec!["add", "-A", "--"];
    add.extend(paths);
    git_in(view, &add, None)?;
    let mut commit = vec!["commit", "-q", "-m", message, "--"];
    commit.extend(paths);
    tool_git_in(view, &commit, None)?;
    Ok(())
}

fn run(dir: &Path, args: &[&str]) -> Result<String> {
    git_in(dir, args, None)
}

fn git_in(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<String> {
    git_env(dir, args, stdin, &[], false)
}

fn tool_git_in(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<String> {
    git_env(dir, args, stdin, &[], true)
}

fn git_env(dir: &Path, args: &[&str], stdin: Option<&[u8]>, env: &[(&str, &std::ffi::OsStr)], as_tool: bool) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    if as_tool {
        command.args(["-c", &format!("user.name={TOOL_NAME}"), "-c", &format!("user.email={TOOL_EMAIL}")]);
    }
    command.args(args).envs(env.iter().copied()).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().with_context(|| format!("running git {args:?}"))?;
    if let Some(bytes) = stdin {
        child.stdin.take().ok_or_else(|| anyhow!("git {args:?}: no stdin"))?.write_all(bytes)?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}
