use anyhow::{bail, Context, Result};
use muckpile_core::body::Loss;
use muckpile_core::identity::load_identity;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, id] if cmd == "to-work" => run_to_work(id),
        [cmd] if cmd == "pull" => run_pull(None),
        [cmd, id] if cmd == "pull" => run_pull(Some(id)),
        [cmd, sub] if cmd == "sprint" && sub == "fetch" => run_sprint_fetch(),
        [cmd, id, status] if cmd == "transition" => run_transition(id, status),
        [cmd, sub] if cmd == "states" && sub == "discover" => run_states_discover(),
        [cmd, rest @ ..] if cmd == "list" => run_list(rest),
        [cmd, view] if cmd == "status" => run_status(view),
        [cmd, view] if cmd == "push" => run_push(view),
        [cmd, a, phrase, b] if cmd == "link" => run_link(a, phrase, b),
        [cmd, a, phrase, b] if cmd == "unlink" => run_unlink(a, phrase, b),
        [cmd, id, new_title] if cmd == "title" => run_title(id, new_title),
        [cmd, id, parent_id] if cmd == "parent" => run_parent(id, parent_id),
        [cmd, id, file, rest @ ..] if cmd == "comment" => run_comment(id, file, rest),
        [cmd, id, file] if cmd == "attach" => run_attach(id, file),
        [cmd, item_type, title, rest @ ..] if cmd == "new" => run_new(item_type, title, rest),
        [cmd, id] if cmd == "show" => run_show(id, false),
        [cmd, id, flag] if cmd == "show" && flag == "--local" => run_show(id, true),
        [cmd, sub, repo, rest @ ..] if cmd == "code-work" && sub == "add" => run_code_work_add(repo, rest),
        _ => bail!(
            "uso: muckpile to-work <id> | muckpile pull [id] | muckpile sprint fetch | muckpile transition <id> <estado> | muckpile states discover | muckpile list <vista> [--state <estado>] [--category <categoria>] [--parent <id>] | muckpile status <vista> | muckpile push <vista> | muckpile link <a> <frase> <b> | muckpile unlink <a> <frase> <b> | muckpile title <id> <título> | muckpile parent <id> <padre> | muckpile comment <id> <archivo> [--reply-to <id>] (--ai <modelo> | --i-human) | muckpile attach <id> <archivo> | muckpile new <tipo> <título> [--parent <id>] [--blocks <id>] | muckpile show <id> [--local] | muckpile code-work add <repo> [--from <rama>] [--branch <rama>]"
        ),
    }
}

fn run_to_work(id: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let view = muckpile_cli::to_work(&root, &cwd, id)?;
    let rel = view.strip_prefix(&root).unwrap_or(&view);
    println!("{}/ creada", rel.display());
    Ok(())
}

fn run_pull(id: Option<&str>) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let pulled = muckpile_cli::pull(&root, &cwd, id, provider.as_ref(), &config)?;
    let rel = pulled.path.strip_prefix(&root).unwrap_or(&pulled.path);
    if pulled.losses.is_empty() {
        println!("{} traído", rel.display());
    } else {
        println!("{} traído — el cuerpo es de sólo lectura: {}", rel.display(), describe_losses(&pulled.losses));
    }
    Ok(())
}

/// One header edit `push` won't send, and the command that makes it — as help:
/// the file is left as it is either way.
fn describe_header_edit(id: &str, edit: &HeaderEdit) -> String {
    let shown = |v: &Option<String>| v.as_deref().map(|v| format!("\"{v}\"")).unwrap_or_else(|| "nada".to_string());
    match edit {
        HeaderEdit::Field { name, written, provider } => {
            let command = match (name.as_str(), written) {
                ("title", Some(v)) => format!("muckpile title {id} \"{v}\""),
                ("status", Some(v)) => format!("muckpile transition {id} \"{v}\""),
                ("parent", Some(v)) => format!("muckpile parent {id} {v}"),
                _ => "ningún comando lo borra".to_string(),
            };
            format!("{name}: {} (el proveedor tiene {}) — {command}", shown(written), shown(provider))
        }
        HeaderEdit::Relation { phrase, other, added: true } => format!("relation.{phrase}: {other} agregada — muckpile link {id} {phrase} {other}"),
        HeaderEdit::Relation { phrase, other, added: false } => format!("relation.{phrase}: {other} quitada — muckpile unlink {id} {phrase} {other}"),
    }
}

/// Every reason a body can't be edited locally, on one line.
fn describe_losses(losses: &[Loss]) -> String {
    losses
        .iter()
        .map(|loss| match loss {
            Loss::Lossy(construct) => format!("el conversor sólo puede aproximar {construct}"),
            Loss::Differs => "no vuelve igual por markdown".to_string(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn run_sprint_fetch() -> Result<()> {
    let (root, _cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let result = muckpile_cli::sprint_fetch(&root, provider.as_ref(), &config)?;

    let project = root.file_name().context("la raíz del proyecto no tiene nombre")?.to_string_lossy().into_owned();
    println!("{project}: {} sprint(s) abierto(s)", result.open);
    for slug in &result.created {
        println!("  backlog/sprint/{slug}/       creada, vacía");
    }
    for slug in &result.removed {
        println!("  backlog/sprint/{slug}/       borrada, ya no está abierto y no tenía nada adentro");
    }
    Ok(())
}

fn run_transition(id: &str, target_status: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    match muckpile_cli::transition(id, target_status, provider.as_ref())? {
        Outcome::Applied { transition_name } => {
            println!("{id}: transición \"{transition_name}\" -> {target_status}");
            catch_up_view(&cwd, id, provider.as_ref(), &config)?;
        }
        Outcome::NoSuchTransition { available } => {
            println!("{id}: no hay transición hacia \"{target_status}\" — disponibles: {}", available.join(", "));
        }
    }
    Ok(())
}

fn run_states_discover() -> Result<()> {
    let (root, _cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let project = root.file_name().context("la raíz del proyecto no tiene nombre")?.to_string_lossy().into_owned();
    let states = muckpile_cli::states_discover(&root, &project, provider.as_ref(), &config)?;

    println!("{project}: {} estado(s)", states.len());
    for (name, category) in &states {
        println!("  {name}    {category}");
    }
    println!("cacheado en {project}.states.toml");
    Ok(())
}

fn run_list(args: &[String]) -> Result<()> {
    let Some((view_arg, flags)) = args.split_first() else {
        bail!("uso: muckpile list <vista> [--state <estado>] [--category <categoria>] [--parent <id>]");
    };

    let mut state = None;
    let mut category = None;
    let mut parent = None;
    let mut i = 0;
    while i < flags.len() {
        let flag = &flags[i];
        let value = flags.get(i + 1).with_context(|| format!("{flag}: falta el valor"))?;
        match flag.as_str() {
            "--state" => state = Some(value.as_str()),
            "--category" => category = Some(value.as_str()),
            "--parent" => parent = Some(value.as_str()),
            _ => bail!("{flag}: opción desconocida"),
        }
        i += 2;
    }

    let (root, cwd) = standing_in_a_project()?;
    let view = cwd.join(view_arg);

    let categories = match category {
        Some(_) => {
            let project = root.file_name().context("la raíz del proyecto no tiene nombre")?.to_string_lossy().into_owned();
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
            println!("{}: sin cambios", item.id);
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

fn display_parent(parent: &Option<String>) -> &str {
    parent.as_deref().unwrap_or("(ninguno)")
}

fn run_push(view_arg: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let view = cwd.join(view_arg);

    for outcome in muckpile_cli::push(&view, provider.as_ref(), &config)? {
        match outcome.result {
            PushResult::Unchanged => println!("{}: sin cambios", outcome.id),
            PushResult::NeverPulled => println!("{}: nunca se hizo pull acá — nada para comparar", outcome.id),
            PushResult::Stale => println!("{}: cambió del otro lado desde tu último pull — no se escribió nada", outcome.id),
            PushResult::Resolved { id, created } => {
                let how = if created { "creado" } else { "encontrado" };
                println!("{}: {how} como {id}", outcome.id);
            }
            PushResult::ResolveFailed(reason) => println!("{}: no se pudo resolver — {reason}", outcome.id),
            PushResult::RelationFailed { phrase, other, reason } => println!(
                "{}: relation.{phrase} {other} no llegó al proveedor — {reason}. Para reintentarlo: muckpile link {} {phrase} {other}",
                outcome.id, outcome.id
            ),
            PushResult::HeaderClash(edits) => {
                println!("{}: el header no es el del proveedor — no se manda nada de este ítem (git diff muestra qué se editó):", outcome.id);
                for edit in &edits {
                    println!("  {}", describe_header_edit(&outcome.id, edit));
                }
            }
            PushResult::Written { body, body_refused } => {
                if body {
                    println!("{}: cuerpo enviado", outcome.id);
                }
                if let Some(refused) = body_refused {
                    println!(
                        "{}: el cuerpo no es canónico ({}) — no se sube. Diff contra el ADF del proveedor:\n{}",
                        outcome.id,
                        describe_losses(&refused.losses),
                        refused.diff
                    );
                } else if !body {
                    println!("{}: sin cambios para enviar", outcome.id);
                }
            }
        }
    }
    Ok(())
}

fn run_title(id: &str, new_title: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    muckpile_cli::title(id, new_title, provider.as_ref())?;
    println!("{id}: título cambiado a \"{new_title}\"");
    catch_up_view(&cwd, id, provider.as_ref(), &config)
}

/// Brings `id` up to what the provider has now in the view `cwd` stands in,
/// after a command wrote it — and says so, or says why the view stays
/// behind. Nothing to say when the view doesn't hold the item.
fn catch_up_view(cwd: &Path, id: &str, provider: &dyn Provider, config: &ProjectConfig) -> Result<()> {
    match muckpile_cli::catch_up(cwd, id, provider, config)? {
        CatchUp::NotHere => {}
        CatchUp::CaughtUp => println!("  {id}: vista al día"),
        CatchUp::Behind(why) => println!("  {id}: la vista queda atrás hasta el próximo pull — {why}"),
    }
    Ok(())
}

fn run_parent(id: &str, parent_id: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    muckpile_cli::parent(id, parent_id, provider.as_ref())?;
    println!("{id}: padre cambiado a {parent_id}");
    catch_up_view(&cwd, id, provider.as_ref(), &config)
}

fn run_comment(id: &str, file: &str, flags: &[String]) -> Result<()> {
    let (mut reply_to, mut model, mut human) = (None, None, false);
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--reply-to" => reply_to = Some(rest.next().context("--reply-to necesita el id de un comentario")?.as_str()),
            "--ai" => model = Some(rest.next().context("--ai necesita el modelo")?.as_str()),
            "--i-human" => human = true,
            other => bail!("{other}: flag desconocido para comment"),
        }
    }
    let author = match (model, human) {
        (Some(_), true) => bail!("--ai y --i-human a la vez: un comentario lo escribió uno de los dos"),
        (Some(model), false) => Some(muckpile_cli::Author::Ai(model)),
        (None, true) => Some(muckpile_cli::Author::Human(muckpile_cli::confirm_human(ask_on_terminal)?)),
        (None, false) => None,
    };
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let comment_id = muckpile_cli::comment(id, &cwd.join(file), reply_to, author, provider.as_ref())?;
    println!("{id}: comentario {comment_id} agregado");
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
    let mut out = std::fs::OpenOptions::new().write(true).open(output).with_context(|| format!("abriendo {output}"))?;
    write!(out, "Para confirmar que lo escribió una persona, escribí: {phrase}\n> ")?;
    out.flush()?;
    let mut answer = String::new();
    std::io::BufReader::new(std::fs::File::open(input).with_context(|| format!("abriendo {input}"))?).read_line(&mut answer)?;
    Ok(answer)
}

fn run_attach(id: &str, file: &str) -> Result<()> {
    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    let attachment = muckpile_cli::attach(id, &cwd.join(file), provider.as_ref())?;
    println!("{id}: {} adjuntado ({})", attachment.filename, attachment.id);
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
            println!("\"{phrase}\": no es una frase de relación del proveedor — disponibles: {}", available.join(", "));
        }
    }
    Ok(())
}

fn run_unlink(a: &str, phrase: &str, b: &str) -> Result<()> {
    let (root, _cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    match muckpile_cli::unlink(a, phrase, b, provider.as_ref())? {
        UnlinkOutcome::Removed { type_name } => println!("{a} {phrase} {b}  ({type_name}) — quitada"),
        UnlinkOutcome::NoSuchPhrase { available } => {
            println!("\"{phrase}\": no es una frase de relación del proveedor — disponibles: {}", available.join(", "));
        }
        UnlinkOutcome::NoSuchLink { type_name } => println!("{a} {phrase} {b}: el proveedor no tiene esa relación ({type_name}) — nada que quitar"),
    }
    Ok(())
}

fn run_new(item_type: &str, title: &str, flags: &[String]) -> Result<()> {
    let mut parent = None;
    let mut blocks = None;
    let mut i = 0;
    while i < flags.len() {
        let flag = &flags[i];
        let value = flags.get(i + 1).with_context(|| format!("{flag}: falta el valor"))?;
        match flag.as_str() {
            "--parent" => parent = Some(value.as_str()),
            "--blocks" => blocks = Some(value.as_str()),
            _ => bail!("{flag}: opción desconocida"),
        }
        i += 2;
    }

    let cwd = std::env::current_dir()?;
    let path = muckpile_cli::new(&cwd, item_type, title, parent, blocks)?;
    let name = path.file_name().context("el path creado no tiene nombre")?.to_string_lossy();
    println!("{name} creado");
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
        let value = flags.get(i + 1).with_context(|| format!("{flag}: falta el valor"))?;
        match flag.as_str() {
            "--from" => from = Some(value.as_str()),
            "--branch" => branch = Some(value.as_str()),
            _ => bail!("{flag}: opción desconocida"),
        }
        i += 2;
    }

    let (root, cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let worktree = muckpile_cli::code_work_add(&root, &cwd, repo, from, branch, &config)?;
    let rel = worktree.strip_prefix(&root).unwrap_or(&worktree);
    println!("{}/ creado", rel.display());
    Ok(())
}

fn standing_in_a_project() -> Result<(PathBuf, PathBuf)> {
    let cwd = std::env::current_dir()?;
    let Some(root) = find_project_root(&cwd) else {
        bail!("no estás parado dentro de un proyecto de muckpile");
    };
    Ok((root, cwd))
}

/// Builds the provider `config` names — today only `jira-rest`, reached with
/// the credentials this machine's `identity.toml` has for this project.
fn build_provider(root: &Path, config: &ProjectConfig) -> Result<Box<dyn Provider>> {
    if config.provider != "jira-rest" {
        bail!("{}: proveedor sin soporte", config.provider);
    }
    let project = root.file_name().context("la raíz del proyecto no tiene nombre")?.to_string_lossy().into_owned();
    let home = std::env::var("HOME").context("no hay HOME en el entorno")?;
    let identity_path = Path::new(&home).join(".config/muckpile/identity.toml");
    let identity = load_identity(&identity_path, &project)?;
    let token = std::env::var(&identity.jira_token_env)
        .with_context(|| format!("{}: variable de entorno no seteada", identity.jira_token_env))?;
    let creds = Credentials::new(identity.jira_email, token);
    Ok(Box::new(JiraRest::new(&config.jira_base_url, creds)))
}
