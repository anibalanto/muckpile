//! The `muckpile` binary's commands, as plain functions over a project root
//! and a working directory. `main.rs` only parses argv and prints; every
//! rule that needs a test lives here instead.

use anyhow::{bail, Context, Result};
use muckpile_core::is_valid_id;
use muckpile_core::project::require_root;
use std::path::{Path, PathBuf};

/// Assembles a working view: `<root>/to-work/<id>/`, empty. Fetching the
/// item is `pull`'s job, and it doesn't exist yet — this only makes the
/// place for it to land.
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
