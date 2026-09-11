use anyhow::{bail, Context, Result};
use muckpile_core::body::Loss;
use muckpile_core::i18n::{lang_from_env, set_lang};
use muckpile_core::identity::load_identity;
use muckpile_core::msg;
use muckpile_core::project::{find_project_root, load_project_config, ProjectConfig};
use muckpile_core::states::read_states_cache;
use muckpile_cli::{CatchUp, HeaderEdit, ListFilter, PushResult};
use muckpile_provider::link::{Outcome as LinkOutcome, UnlinkOutcome};
use muckpile_provider::provider::Provider;
use muckpile_provider::rest::{Credentials, JiraRest};
use muckpile_provider::transition::Outcome;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    set_lang(lang_from_env());
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, id] if cmd == "to-work" => run_to_work(id, false),
        [cmd, id, flag] if cmd == "to-work" && flag == "--empty" => run_to_work(id, true),
        [cmd, rest @ ..] if cmd == "pull" => run_pull(rest),
        [cmd, sub] if cmd == "sprint" && sub == "fetch" => run_sprint_fetch(),
        [cmd, id, status] if cmd == "transition" => run_transition(id, status),
        [cmd, sub] if cmd == "states" && sub == "discover" => run_states_discover(),
        [cmd, rest @ ..] if cmd == "list" => run_list(rest),
        [cmd, view] if cmd == "status" => run_status(view),
        [cmd, view] if cmd == "push" => run_push(view),
        [cmd, a, phrase, b] if cmd == "link" => run_link(a, phrase, b),
        [cmd, a, phrase, b] if cmd == "unlink" => run_unlink(a, phrase, b),
        [cmd, id, new_title] if cmd == "title" => run_title(id, new_title),
        [cmd, name] if cmd == "init" => run_init(name),
        [cmd, id, parent_id] if cmd == "parent" => run_parent(id, parent_id),
        [cmd, id, file, rest @ ..] if cmd == "comment" => run_comment(id, file, rest),
        [cmd, id, file] if cmd == "attach" => run_attach(id, file),
        [cmd, item_type, title, rest @ ..] if cmd == "new" => run_new(item_type, title, rest),
        [cmd, id] if cmd == "show" => run_show(id, false),
        [cmd, id, flag] if cmd == "show" && flag == "--local" => run_show(id, true),
        [cmd, sub, repo, rest @ ..] if cmd == "code-work" && sub == "add" => run_code_work_add(repo, rest),
        _ => bail!(msg!("usage.all")),
    }
}

fn run_to_work(id: &str, empty: bool) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    if empty {
        let view = muckpile_cli::to_work(&root, &cwd, id)?;
        let rel = view.strip_prefix(&root).unwrap_or(&view);
        println!("{}", msg!("to_work.created_empty", view = rel.display()));
        return Ok(());
    }
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let pulled = muckpile_cli::to_work_and_pull(&root, &cwd, id, provider.as_ref(), &config)?;
    print_pulled(&root, &pulled);
    Ok(())
}

fn run_pull(args: &[String]) -> Result<()> {
    let (mut target, mut query) = (None, None);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--query" => query = Some(rest.next().with_context(|| msg!("pull.query.needs_value"))?.as_str()),
            other if target.is_none() => target = Some(other),
            other => bail!(msg!("pull.extra_argument", arg = other)),
        }
    }
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let membership = if let Some(view) = muckpile_cli::query_view_of(&root, &cwd, target) {
        Some((view.clone(), muckpile_cli::pull_query(&root, &view, query, provider.as_ref(), &config)?, msg!("pull.gone.query")))
    } else if query.is_some() {
        bail!(msg!("pull.query.only_in_query_view"));
    } else if let Some(view) = muckpile_cli::sprint_view_of(&root, &cwd, target) {
        Some((view.clone(), muckpile_cli::pull_sprint(&root, &view, provider.as_ref(), &config)?, msg!("pull.gone.sprint")))
    } else {
        None
    };
    if let Some((view, pulled, why_gone)) = membership {
        let name = view.strip_prefix(&root).unwrap_or(&view).display().to_string();
        println!("{}", msg!("pull.view.brought", view = name, count = pulled.items.len()));
        for item in &pulled.items {
            print_pulled(&root, item);
        }
        for id in &pulled.gone {
            println!("  {}", msg!("pull.view.gone", id, why = why_gone));
        }
        return Ok(());
    }
    let pulled = muckpile_cli::pull(&root, &cwd, target, provider.as_ref(), &config)?;
    print_pulled(&root, &pulled);
    Ok(())
}

/// One item `pull` brought: where it landed, whether its body is read-only,
/// and which other views on this machine already hold it.
fn print_pulled(root: &Path, pulled: &muckpile_cli::Pulled) {
    let rel = pulled.path.strip_prefix(root).unwrap_or(&pulled.path);
    if pulled.losses.is_empty() {
        println!("{}", msg!("pull.brought", path = rel.display()));
    } else {
        println!("{}", msg!("pull.brought_read_only", path = rel.display(), losses = describe_losses(&pulled.losses)));
    }
    if !pulled.also_in.is_empty() {
        println!("  {}", msg!("pull.also_in", views = pulled.also_in.join(", ")));
    }
}

/// One header edit `push` won't send, and the command that makes it — as help:
/// the file is left as it is either way.
fn describe_header_edit(id: &str, edit: &HeaderEdit) -> String {
    let shown = |v: &Option<String>| v.as_deref().map(|v| format!("\"{v}\"")).unwrap_or_else(|| msg!("push.clash.nothing"));
    match edit {
        HeaderEdit::Field { name, written, provider } => {
            let command = match (name.as_str(), written) {
                ("title", Some(v)) => format!("muckpile title {id} \"{v}\""),
                ("status", Some(v)) => format!("muckpile transition {id} \"{v}\""),
                ("parent", Some(v)) => format!("muckpile parent {id} {v}"),
                _ => msg!("push.clash.no_command"),
            };
            msg!("push.clash.field", name, written = shown(written), provider = shown(provider), command)
        }
        HeaderEdit::Relation { phrase, other, added: true } => msg!("push.clash.relation_added", phrase, other, id),
        HeaderEdit::Relation { phrase, other, added: false } => msg!("push.clash.relation_removed", phrase, other, id),
    }
}

/// Every reason a body can't be edited locally, on one line.
fn describe_losses(losses: &[Loss]) -> String {
    losses
        .iter()
        .map(|loss| match loss {
            Loss::Lossy(construct) => msg!("loss.lossy", construct),
            Loss::Differs => msg!("loss.differs"),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn run_sprint_fetch() -> Result<()> {
    let (root, _cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let result = muckpile_cli::sprint_fetch(&root, provider.as_ref(), &config)?;

    let project = root.file_name().with_context(|| msg!("project.root_unnamed"))?.to_string_lossy().into_owned();
    println!("{}", msg!("sprint_fetch.open", project, count = result.open));
    for slug in &result.created {
        println!("  {}", msg!("sprint_fetch.created", slug));
    }
    for slug in &result.removed {
        println!("  {}", msg!("sprint_fetch.removed", slug));
    }
    Ok(())
}

fn run_transition(id: &str, target_status: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    match muckpile_cli::transition(id, target_status, provider.as_ref())? {
        Outcome::Applied { transition_name } => {
            println!("{}", msg!("transition.applied", id, transition = transition_name, status = target_status));
            catch_up_view(&cwd, id, provider.as_ref(), &config)?;
        }
        Outcome::NoSuchTransition { available } => {
            println!("{}", msg!("transition.none", id, status = target_status, available = available.join(", ")));
        }
    }
    Ok(())
}

fn run_states_discover() -> Result<()> {
    let (root, _cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let project = root.file_name().with_context(|| msg!("project.root_unnamed"))?.to_string_lossy().into_owned();
    let states = muckpile_cli::states_discover(&root, &project, provider.as_ref(), &config)?;

    println!("{}", msg!("states.count", project, count = states.len()));
    for (name, category) in &states {
        println!("  {name}    {category}");
    }
    println!("{}", msg!("states.cached", project));
    Ok(())
}

fn run_list(args: &[String]) -> Result<()> {
    let Some((view_arg, flags)) = args.split_first() else {
        bail!(msg!("usage.list"));
    };

    let mut state = None;
    let mut category = None;
    let mut parent = None;
    let mut i = 0;
    while i < flags.len() {
        let flag = &flags[i];
        let value = flags.get(i + 1).with_context(|| msg!("flag.missing_value", flag))?;
        match flag.as_str() {
            "--state" => state = Some(value.as_str()),
            "--category" => category = Some(value.as_str()),
            "--parent" => parent = Some(value.as_str()),
            _ => bail!(msg!("flag.unknown_option", flag)),
        }
        i += 2;
    }

    let (root, cwd) = standing_in_a_project()?;
    let view = cwd.join(view_arg);

    let categories = match category {
        Some(_) => {
            let project = root.file_name().with_context(|| msg!("project.root_unnamed"))?.to_string_lossy().into_owned();
            read_states_cache(&root.join(format!("{project}.states.toml")))?
        }
        None => BTreeMap::new(),
    };

    let filter = ListFilter { state, category, parent };
    let items = muckpile_cli::list(&view, &filter, &categories)?;
    for item in &items {
        println!("{}  {}  {}  {}", item.id, item.item_type, item.status, item.title);
    }
    Ok(())
}

fn run_status(view_arg: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let view = cwd.join(view_arg);

    for item in muckpile_cli::status(&view, provider.as_ref())? {
        if !item.changed {
            println!("{}", msg!("item.unchanged", id = item.id));
            continue;
        }
        let mut diffs = Vec::new();
        if item.local_title != item.remote_title {
            diffs.push(format!("title \"{}\" -> \"{}\"", item.local_title, item.remote_title));
        }
        if item.local_status != item.remote_status {
            diffs.push(format!("status \"{}\" -> \"{}\"", item.local_status, item.remote_status));
        }
        if item.local_parent != item.remote_parent {
            diffs.push(format!("parent {} -> {}", display_parent(&item.local_parent), display_parent(&item.remote_parent)));
        }
        println!("{}: {}", item.id, diffs.join(", "));
    }
    Ok(())
}

fn display_parent(parent: &Option<String>) -> String {
    parent.clone().unwrap_or_else(|| msg!("status.no_parent"))
}

fn run_push(view_arg: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let view = cwd.join(view_arg);

    for outcome in muckpile_cli::push(&view, provider.as_ref(), &config)? {
        match outcome.result {
            PushResult::Unchanged => println!("{}", msg!("item.unchanged", id = outcome.id)),
            PushResult::NeverPulled => println!("{}", msg!("push.never_pulled", id = outcome.id)),
            PushResult::Stale => println!("{}", msg!("push.stale", id = outcome.id)),
            PushResult::Resolved { id, created } => {
                let key = if created { "push.resolve.created" } else { "push.resolve.found" };
                println!("{}", msg!(key, slug = outcome.id, id));
            }
            PushResult::ResolveFailed(reason) => println!("{}", msg!("push.resolve.failed", slug = outcome.id, reason)),
            PushResult::Local => println!("{}", msg!("push.local", slug = outcome.id)),
            PushResult::RelationFailed { phrase, other, reason } => println!("{}", msg!("push.relation.failed", id = outcome.id, phrase, other, reason)),
            PushResult::HeaderClash(edits) => {
                println!("{}", msg!("push.clash.header", id = outcome.id));
                for edit in &edits {
                    println!("  {}", describe_header_edit(&outcome.id, edit));
                }
            }
            PushResult::Written { body, body_refused } => {
                if body {
                    println!("{}", msg!("push.body_sent", id = outcome.id));
                }
                if let Some(refused) = body_refused {
                    println!("{}", msg!("push.body_refused", id = outcome.id, losses = describe_losses(&refused.losses), diff = refused.diff));
                } else if !body {
                    println!("{}", msg!("push.nothing_to_send", id = outcome.id));
                }
            }
        }
    }
    Ok(())
}

fn run_init(name: &str) -> Result<()> {
    let project = muckpile_cli::init(&std::env::current_dir()?, name)?;
    println!("{}", msg!("init.created", project = project.display(), name));
    Ok(())
}

fn run_title(id: &str, new_title: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    muckpile_cli::title(id, new_title, provider.as_ref())?;
    println!("{}", msg!("title.changed", id, title = new_title));
    catch_up_view(&cwd, id, provider.as_ref(), &config)
}

/// Brings `id` up to what the provider has now in the view `cwd` stands in,
/// after a command wrote it — and says so, or says why the view stays
/// behind. Nothing to say when the view doesn't hold the item.
fn catch_up_view(cwd: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<()> {
    match muckpile_cli::catch_up(cwd, id, provider, config)? {
        CatchUp::NotHere => {}
        CatchUp::CaughtUp => println!("  {}", msg!("catch_up.caught_up", id)),
        CatchUp::Behind(why) => println!("  {}", msg!("catch_up.behind", id, why)),
    }
    Ok(())
}

fn run_parent(id: &str, parent_id: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    muckpile_cli::parent(id, parent_id, provider.as_ref())?;
    println!("{}", msg!("parent.changed", id, parent = parent_id));
    catch_up_view(&cwd, id, provider.as_ref(), &config)
}

fn run_comment(id: &str, file: &str, flags: &[String]) -> Result<()> {
    let (mut reply_to, mut model, mut human) = (None, None, false);
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--reply-to" => reply_to = Some(rest.next().with_context(|| msg!("comment.reply_to.needs_value"))?.as_str()),
            "--ai" => model = Some(rest.next().with_context(|| msg!("comment.ai.needs_value"))?.as_str()),
            "--i-human" => human = true,
            other => bail!(msg!("comment.unknown_flag", flag = other)),
        }
    }
    let author = match (model, human) {
        (Some(_), true) => bail!(msg!("comment.ai_and_human")),
        (Some(model), false) => Some(muckpile_cli::Author::Ai(model)),
        (None, true) => Some(muckpile_cli::Author::Human(muckpile_cli::confirm_human(ask_on_terminal)?)),
        (None, false) => None,
    };
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let comment_id = muckpile_cli::comment(id, &cwd.join(file), reply_to, author, provider.as_ref())?;
    println!("{}", msg!("comment.added", id, comment = comment_id));
    catch_up_view(&cwd, id, provider.as_ref(), &config)
}

/// Shows `phrase` on the terminal and reads the answer from it — the
/// terminal itself, never standard input, so a pipe can't answer.
fn ask_on_terminal(phrase: &str) -> Result<String> {
    use std::io::{BufRead, Write};
    #[cfg(windows)]
    let (input, output) = ("CONIN$", "CONOUT$");
    #[cfg(not(windows))]
    let (input, output) = ("/dev/tty", "/dev/tty");
    let mut out = std::fs::OpenOptions::new().write(true).open(output).with_context(|| msg!("terminal.opening", path = output))?;
    write!(out, "{}", msg!("comment.human.prompt", phrase))?;
    out.flush()?;
    let mut answer = String::new();
    std::io::BufReader::new(std::fs::File::open(input).with_context(|| msg!("terminal.opening", path = input))?).read_line(&mut answer)?;
    Ok(answer)
}

fn run_attach(id: &str, file: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let attachment = muckpile_cli::attach(id, &cwd.join(file), provider.as_ref())?;
    println!("{}", msg!("attach.done", id, filename = attachment.filename, attachment = attachment.id));
    catch_up_view(&cwd, id, provider.as_ref(), &config)
}

fn run_link(a: &str, phrase: &str, b: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    match muckpile_cli::link(a, phrase, b, provider.as_ref())? {
        LinkOutcome::Applied { type_name } => {
            println!("{a} {phrase} {b}  ({type_name})");
            // A link shows on both of its ends.
            catch_up_view(&cwd, a, provider.as_ref(), &config)?;
            catch_up_view(&cwd, b, provider.as_ref(), &config)?;
        }
        LinkOutcome::NoSuchPhrase { available } => {
            println!("{}", msg!("link.no_such_phrase", phrase, available = available.join(", ")));
        }
    }
    Ok(())
}

fn run_unlink(a: &str, phrase: &str, b: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    match muckpile_cli::unlink(a, phrase, b, provider.as_ref())? {
        UnlinkOutcome::Removed { type_name } => {
            println!("{}", msg!("unlink.removed", a, phrase, b, type_name));
            // A link shows on both of its ends.
            catch_up_view(&cwd, a, provider.as_ref(), &config)?;
            catch_up_view(&cwd, b, provider.as_ref(), &config)?;
        }
        UnlinkOutcome::NoSuchPhrase { available } => {
            println!("{}", msg!("link.no_such_phrase", phrase, available = available.join(", ")));
        }
        UnlinkOutcome::NoSuchLink { type_name } => println!("{}", msg!("unlink.no_such_link", a, phrase, b, type_name)),
    }
    Ok(())
}

fn run_new(item_type: &str, title: &str, flags: &[String]) -> Result<()> {
    let mut parent = None;
    let mut blocks = None;
    let mut i = 0;
    while i < flags.len() {
        let flag = &flags[i];
        let value = flags.get(i + 1).with_context(|| msg!("flag.missing_value", flag))?;
        match flag.as_str() {
            "--parent" => parent = Some(value.as_str()),
            "--blocks" => blocks = Some(value.as_str()),
            _ => bail!(msg!("flag.unknown_option", flag)),
        }
        i += 2;
    }

    let cwd = std::env::current_dir()?;
    let path = muckpile_cli::new(&cwd, item_type, title, parent, blocks)?;
    let name = path.file_name().with_context(|| msg!("new.unnamed_path"))?.to_string_lossy();
    println!("{}", msg!("path.created", path = name));
    Ok(())
}

fn run_show(id: &str, local: bool) -> Result<()> {
    let show = if local {
        let cwd = std::env::current_dir()?;
        muckpile_cli::show_local(&cwd, id)?
    } else {
        let (root, cwd) = standing_in_a_project()?;
        let config = load_project_config(&root)?;
        let provider = build_provider(&root, &config)?;
        muckpile_cli::show_live(&cwd, id, provider.as_ref(), &config)?
    };

    println!("title: {}", show.title);
    if let Some(status) = &show.status {
        println!("status: {status}");
    }
    if let Some(parent) = &show.parent {
        println!("parent: {parent}");
    }
    if !show.data_files.is_empty() {
        println!("{id}_data/: {}", show.data_files.join(", "));
    }
    println!();
    println!("{}", show.body);
    Ok(())
}

fn run_code_work_add(repo: &str, flags: &[String]) -> Result<()> {
    let mut from = None;
    let mut branch = None;
    let mut i = 0;
    while i < flags.len() {
        let flag = &flags[i];
        let value = flags.get(i + 1).with_context(|| msg!("flag.missing_value", flag))?;
        match flag.as_str() {
            "--from" => from = Some(value.as_str()),
            "--branch" => branch = Some(value.as_str()),
            _ => bail!(msg!("flag.unknown_option", flag)),
        }
        i += 2;
    }

    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let worktree = muckpile_cli::code_work_add(&root, &cwd, repo, from, branch, &config)?;
    let rel = worktree.strip_prefix(&root).unwrap_or(&worktree);
    println!("{}", msg!("path.created", path = format!("{}/", rel.display())));
    Ok(())
}

fn standing_in_a_project() -> Result<(PathBuf, PathBuf)> {
    let cwd = std::env::current_dir()?;
    let Some(root) = find_project_root(&cwd) else {
        bail!(msg!("project.not_inside"));
    };
    Ok((root, cwd))
}

/// Builds the provider `config` names — today only `jira-rest`, reached with
/// the credentials this machine's `identity.toml` has for this project.
fn build_provider(root: &Path, config: &ProjectConfig) -> Result<Box<dyn Provider>> {
    if config.provider != "jira-rest" {
        bail!(msg!("provider.unsupported", provider = config.provider));
    }
    let project = root.file_name().with_context(|| msg!("project.root_unnamed"))?.to_string_lossy().into_owned();
    let home = std::env::var("HOME").with_context(|| msg!("env.no_home"))?;
    let identity_path = Path::new(&home).join(".config/muckpile/identity.toml");
    let identity = load_identity(&identity_path, &project)?;
    let token = std::env::var(&identity.jira_token_env)
        .with_context(|| msg!("env.unset", var = identity.jira_token_env))?;
    let creds = Credentials::new(identity.jira_email, token);
    Ok(Box::new(JiraRest::new(&config.jira_base_url, creds)))
}
