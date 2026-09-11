//! `<project>.states.toml` — what `states discover` caches: the workflow's
//! own status names mapped to their provider category. Sibling
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

/// Reads a cache `states discover` already wrote. No default when it's
/// missing: a `--category` filter with no cache to consult isn't a category
/// of zero items, it's a question nobody answered yet.
pub fn read_states_cache(path: &Path) -> Result<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| crate::msg!("states.no_cache", path = path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
