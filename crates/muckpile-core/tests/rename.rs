//! `rename_one` against a real git repo in a temp directory. No test touches
//! the project's own repo or any provider.

use std::path::Path;
use std::process::Command;

fn git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path(), &["init", "-q"]);
    run(dir.path(), &["config", "user.email", "test@test"]);
    run(dir.path(), &["config", "user.name", "test"]);
    dir
}

fn run(repo: &Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(repo).args(args).status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn write(repo: &Path, name: &str, content: &str) {
    if let Some(parent) = repo.join(name).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(repo.join(name), content).unwrap();
}

fn head(repo: &Path) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(["rev-parse", "HEAD"]).output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn rename_without_incoming_references() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "slug-alone.task.md", "---\ntitle: Alone\nstatus: Open\n---\nNothing references it.\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);

    let touched = muckpile_core::rename_one(repo, "slug-alone", "ACC-1").unwrap();
    assert!(touched.is_empty());
    assert!(repo.join("ACC-1.task.md").exists());
    assert!(!repo.join("slug-alone.task.md").exists());
}

#[test]
fn rename_rewrites_delimited_references_and_not_substrings() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "slug-a.task.md", "---\ntitle: A\nstatus: Open\n---\n# A\n");
    write(
        repo,
        "slug-b.task.md",
        "---\ntitle: B\nstatus: Open\nrelation.depends: [slug-a]\n---\nDepends on [`slug-a`](slug-a.task.md).\n",
    );
    write(repo, "unrelated.task.md", "---\ntitle: Untouched\nstatus: Open\n---\nMentions slug-a10 and slug-a loose, no backticks.\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);

    let touched = muckpile_core::rename_one(repo, "slug-a", "ACC-101").unwrap();
    assert_eq!(touched, vec!["slug-b.task.md".to_string()]);

    let b = std::fs::read_to_string(repo.join("slug-b.task.md")).unwrap();
    assert!(b.contains("relation.depends: [ACC-101]"));
    assert!(b.contains("[`ACC-101`](ACC-101.task.md)"));

    let unrelated = std::fs::read_to_string(repo.join("unrelated.task.md")).unwrap();
    assert!(unrelated.contains("slug-a10"), "must not touch the substring");
    assert!(unrelated.contains("slug-a loose"), "must not touch the loose, backtick-less mention");
}

#[test]
fn the_sibling_data_directory_travels_with_the_rename() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "slug-q.question.md", "---\ntitle: Q\nstatus: Open\n---\nBody.\n");
    write(repo, "slug-q_data/thread/1.md", "first message\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);

    muckpile_core::rename_one(repo, "slug-q", "ACC-9").unwrap();
    assert!(!repo.join("slug-q_data").exists());
    assert!(repo.join("ACC-9_data/thread/1.md").exists());
}

#[test]
fn a_cycle_is_rejected_without_writing_anything() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "slug-e.task.md", "---\ntitle: E\nstatus: Open\nrelation.depends: [slug-f]\n---\nCycle.\n");
    write(repo, "slug-f.task.md", "---\ntitle: F\nstatus: Open\nrelation.depends: [slug-e]\n---\nCycle.\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);
    let before = head(repo);

    let result = muckpile_core::topo_order(repo, &["slug-e".to_string(), "slug-f".to_string()]);

    assert!(result.is_err());
    assert_eq!(before, head(repo), "the repo must not have changed");
    assert!(repo.join("slug-e.task.md").exists());
    assert!(repo.join("slug-f.task.md").exists());
}

/// A `blocks` relation is just another `relation.*` field: `question`
/// doesn't need special-casing to have its blocking reference rewritten.
#[test]
fn a_blocks_relation_is_rewritten_like_any_other() {
    let text = "---\ntitle: Does the role inherit?\nrelation.blocks: [slug-x]\n---\n\nbody\n";
    let (out, changed) = muckpile_core::rewrite_references(text, "slug-x", "task", "ACC-229");
    assert!(changed);
    assert!(out.contains("relation.blocks: [ACC-229]"), "{out:?}");
}

/// A relation's key is the provider's phrase with `_` for each space, and
/// nothing else changed — so it can carry an accent or a capital, and it's
/// still a relation.
#[test]
fn a_relation_whose_key_is_a_phrase_with_accents_is_rewritten() {
    let text = "---\ntitle: x\nrelation.está_bloqueada_por: [slug-x]\n---\n\nbody\n";
    let (out, changed) = muckpile_core::rewrite_references(text, "slug-x", "task", "ACC-229");
    assert!(changed);
    assert!(out.contains("relation.está_bloqueada_por: [ACC-229]"), "{out:?}");
}

#[test]
fn the_refs_of_a_relation_whose_key_is_a_phrase_are_read() {
    let text = "---\ntitle: x\nrelation.is_blocked_by: [@a, ACC-9]\nrelation.Está_Clonada_por: [@b]\n---\n";
    let refs = muckpile_core::read_frontmatter_refs(text);
    for id in ["@a", "ACC-9", "@b"] {
        assert!(refs.contains(id), "{id} missing from {refs:?}");
    }
}

#[test]
fn renaming_the_parent_keeps_the_newline() {
    let text = "---\ntitle: X\nparent: 1\n---\n\n# X\n\nbody\n";
    let (out, changed) = muckpile_core::rewrite_references(text, "1", "epic", "ACC-14");
    assert!(changed);
    assert_eq!(out, "---\ntitle: X\nparent: ACC-14\n---\n\n# X\n\nbody\n");
}

#[test]
fn the_frontmatter_still_splits_after_renaming_the_parent() {
    let text = "---\ntitle: X\nstatus: Done\nparent: 1\n---\n\n# X\n\nbody\n";
    let (out, _) = muckpile_core::rewrite_references(text, "1", "epic", "ACC-14");
    let (fm, body) = muckpile_core::body::split_frontmatter(&out);
    assert!(fm.contains("parent: ACC-14"), "the frontmatter is the frontmatter: {fm:?}");
    assert!(!fm.is_empty(), "nothing was split: the whole file would travel as body");
    assert_eq!(body, "\n# X\n\nbody\n", "and the body is only the body");
}

#[test]
fn renaming_a_slug_does_not_touch_one_that_ends_with_it() {
    let text = "---\ntitle: k\nrelation.depends: [2j]\n---\n\nbody\n";
    let (out, changed) = muckpile_core::rewrite_references(text, "j", "user-story", "ACC-77");
    assert!(!changed, "there was no reference to `j`");
    assert!(out.contains("[2j]"), "left intact: {out:?}");
}

#[test]
fn the_right_slug_is_rewritten_with_a_lookalike_next_to_it() {
    let text = "---\ntitle: k\nrelation.depends: [2j, j]\n---\n\nbody\n";
    let (out, changed) = muckpile_core::rewrite_references(text, "j", "user-story", "ACC-77");
    assert!(changed);
    assert!(out.contains("[2j, ACC-77]"), "{out:?}");
}

#[test]
fn two_adjacent_references_are_both_rewritten() {
    let text = "---\ntitle: x\nrelation.depends: [j,j]\n---\n\nbody\n";
    let (out, _) = muckpile_core::rewrite_references(text, "j", "task", "ACC-77");
    assert_eq!(out.matches("ACC-77").count(), 2, "both: {out:?}");
    assert!(!out.contains(",j]"), "none left unrewritten: {out:?}");
}

#[test]
fn a_link_with_dotdot_is_rewritten_keeping_the_prefix() {
    let text = "- [`4h` The title](../4h.task.md)\n";
    let (out, changed) = muckpile_core::rewrite_references(text, "4h", "task", "ACC-94");
    assert!(changed);
    assert_eq!(out, "- [`ACC-94` The title](../ACC-94.task.md)\n");
}

/// The rename never crosses into `code-work/`: it's a worktree of the
/// external project, owned by plain git, not by muckpile.
#[test]
fn rename_never_touches_code_work() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "4h.task.md", "---\ntitle: The task\nstatus: Open\n---\n\nbody\n");
    write(repo, "code-work/README.md", "mentions 4h in someone else's repo\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-qm", "seed"]);

    let touched = muckpile_core::rename_one(repo, "4h", "ACC-94").unwrap();
    assert!(!touched.iter().any(|t| t.contains("code-work")), "{touched:?}");
    let readme = std::fs::read_to_string(repo.join("code-work/README.md")).unwrap();
    assert!(readme.contains("mentions 4h"), "code-work must never be rewritten");
}

/// A marked local id keeps the marker as part of the id: `@a` never matches
/// inside `@a1` or `x@a`.
#[test]
fn renaming_a_marked_id_does_not_eat_the_marker_or_touch_its_neighbors() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "@a.task.md", "---\ntitle: A\nstatus: Open\n---\n# A\n");
    write(repo, "@a1.task.md", "---\ntitle: A1\nstatus: Open\n---\n# A1\n");
    write(
        repo,
        "@b.task.md",
        "---\ntitle: B\nstatus: Open\nparent: @a\nrelation.depends: [@a1]\n---\nComes from [`@a`](@a.task.md), and mentions @a1 in passing.\n",
    );
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);

    let touched = muckpile_core::rename_one(repo, "@a", "ACC-347").unwrap();
    assert_eq!(touched, vec!["@b.task.md".to_string()]);
    assert!(repo.join("ACC-347.task.md").exists());
    assert!(!repo.join("@a.task.md").exists());

    let b = std::fs::read_to_string(repo.join("@b.task.md")).unwrap();
    assert!(b.contains("parent: ACC-347"), "{b}");
    assert!(b.contains("[`ACC-347`](ACC-347.task.md)"), "{b}");
    assert!(b.contains("relation.depends: [@a1]"), "the longer lookalike was untouched: {b}");
    assert!(b.contains("mentions @a1 in passing"), "nor its loose mention: {b}");
    assert!(repo.join("@a1.task.md").exists());
}

/// A type change keeps the id and changes only the file's name, so what
/// moves is a link's destination — `parent`, `relation.*` and ids in prose
/// name the id, which stays.
#[test]
fn a_type_change_rewrites_only_the_links_to_the_old_file() {
    let text = "---\ntitle: x\nparent: ACC-355\n---\nSee [the story](ACC-355.task.md), [again](../ACC-355.task.md), and `ACC-355`; not [this](ACC-3555.task.md).\n";
    let (out, changed) = muckpile_core::rewrite_type_references(text, "ACC-355", "task", "user-story");
    assert!(changed);
    assert!(out.contains("[the story](ACC-355.user-story.md)"), "{out}");
    assert!(out.contains("[again](../ACC-355.user-story.md)"), "{out}");
    assert!(out.contains("[this](ACC-3555.task.md)"), "a longer id is another item: {out}");
    assert!(out.contains("parent: ACC-355\n") && out.contains("`ACC-355`"), "{out}");
}

/// The rename commit carries the rename and the references it rewrote —
/// never an edit another view left uncommitted, nor something staged by hand.
#[test]
fn a_rename_commits_only_what_it_touched() {
    let dir = git_repo();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("other-view")).unwrap();
    std::fs::create_dir_all(repo.join("view")).unwrap();
    write(repo, "other-view/ACC-9.task.md", "---\ntitle: untouched\n---\n");
    write(repo, "view/slug-a.task.md", "---\ntitle: A\n---\n");
    write(repo, "view/ACC-1.task.md", "---\ntitle: B\nparent: slug-a\n---\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-qm", "seed"]);
    write(repo, "other-view/ACC-9.task.md", "---\ntitle: mid-edit\n---\n");
    write(repo, "other-view/staged.md", "staged by hand\n");
    run(repo, &["add", "other-view/staged.md"]);

    muckpile_core::rename_one(&repo.join("view"), "slug-a", "ACC-100").unwrap();

    let out = Command::new("git").arg("-C").arg(repo).args(["show", "--name-only", "--no-renames", "--format=", "HEAD"]).output().unwrap();
    let mut committed: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(str::to_string).collect();
    committed.sort();
    assert_eq!(committed, vec!["view/ACC-1.task.md", "view/ACC-100.task.md", "view/slug-a.task.md"]);
}

fn untracked(repo: &Path) -> Vec<String> {
    let out = Command::new("git").arg("-C").arg(repo).args(["ls-files", "--others", "--exclude-standard"]).output().unwrap();
    String::from_utf8(out.stdout).unwrap().lines().map(str::to_string).collect()
}

/// What nobody committed stays that way: a file loose in the draft's
/// `_data/` travels with the rest, and isn't in the rename's commit.
#[test]
fn an_uncommitted_file_in_the_data_directory_moves_but_is_not_committed() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "slug-q.question.md", "---\ntitle: Q\n---\n");
    write(repo, "slug-q_data/thread/1.md", "first message\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);
    write(repo, "slug-q_data/files/notas.md", "sin commitear\n");

    muckpile_core::rename_one(repo, "slug-q", "ACC-9").unwrap();

    assert!(!repo.join("slug-q_data").exists());
    assert!(repo.join("ACC-9_data/thread/1.md").exists());
    assert_eq!(untracked(repo), vec!["ACC-9_data/files/notas.md"]);
}

/// A draft nobody committed that names the renamed one gets its reference
/// rewritten, and stays uncommitted: the rename doesn't commit it for anyone.
#[test]
fn an_uncommitted_draft_that_names_the_renamed_one_is_rewritten_but_not_committed() {
    let dir = git_repo();
    let repo = dir.path();
    write(repo, "@padre.task.md", "---\ntitle: P\n---\n");
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-q", "-m", "seed"]);
    write(repo, "@hijo.task.md", "---\ntitle: H\nparent: @padre\n---\n");

    let touched = muckpile_core::rename_one(repo, "@padre", "ACC-100").unwrap();

    assert_eq!(touched, vec!["@hijo.task.md".to_string()]);
    assert!(std::fs::read_to_string(repo.join("@hijo.task.md")).unwrap().contains("parent: ACC-100\n"));
    assert_eq!(untracked(repo), vec!["@hijo.task.md"]);
}
