use anyhow::{bail, Context, Result};
use muckpile_core::identity::load_identity;
use muckpile_core::project::{find_project_root, load_project_config, ProjectConfig};
use muckpile_provider::provider::Provider;
use muckpile_provider::rest::{Credentials, JiraRest};
use muckpile_provider::transition::Outcome;
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
        _ => bail!(
            "uso: muckpile to-work <id> | muckpile pull [id] | muckpile sprint fetch | muckpile transition <id> <estado> | muckpile states discover"
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
    let path = muckpile_cli::pull(&root, &cwd, id, provider.as_ref(), &config)?;
    let rel = path.strip_prefix(&root).unwrap_or(&path);
    println!("{} traído", rel.display());
    Ok(())
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
    let (root, _cwd) = standing_in_a_project()?;
    let config = load_project_config(&root)?;
    let provider = build_provider(&root, &config)?;
    match muckpile_cli::transition(id, target_status, provider.as_ref())? {
        Outcome::Applied { transition_name } => {
            println!("{id}: transición \"{transition_name}\" -> {target_status}");
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
