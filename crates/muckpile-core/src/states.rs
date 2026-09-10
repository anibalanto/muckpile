//! `<project>.states.toml` — what `states discover` caches: the workflow's
//! own status names mapped to their provider category (decision 8). Sibling
//! to `muckpile.toml`, regenerable, never edited by hand.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Replaces `path`'s content with `states` — the cache is always written
/// whole, never merged with what was there before.
pub fn write_states_cache(path: &Path, states: &BTreeMap<String, String>) -> Result<()> {
    let text = toml::to_string_pretty(states).context("serializing the states cache")?;
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}
