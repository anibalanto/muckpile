use anyhow::{bail, Result};
use muckpile_core::project::find_project_root;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, id] if cmd == "to-work" => run_to_work(id),
        _ => bail!("uso: muckpile to-work <id>"),
    }
}

fn run_to_work(id: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let Some(root) = find_project_root(&cwd) else {
        bail!("no estás parado dentro de un proyecto de muckpile");
    };
    let view = muckpile_cli::to_work(&root, &cwd, id)?;
    let rel = view.strip_prefix(&root).unwrap_or(&view);
    println!("{}/ creada", rel.display());
    Ok(())
}
