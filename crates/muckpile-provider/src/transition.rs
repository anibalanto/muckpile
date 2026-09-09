//! Deciding which transition fires for a requested status, and firing it.
//! Transport-agnostic: it only knows the `Provider` port, so it runs the
//! same against the fake and against the real thing.

use crate::provider::Provider;
use anyhow::Result;

pub enum Outcome {
    /// The transition applied. `transition_name` is what the workflow calls
    /// the transition itself, which is not the status it leads to.
    Applied { transition_name: String },
    /// No transition from here leads to the requested status. Not a
    /// transport error — the answer, with what the workflow does allow.
    NoSuchTransition { available: Vec<String> },
}

pub fn transition(provider: &dyn Provider, key: &str, target_status: &str) -> Result<Outcome> {
    let available = provider.transitions(key)?;
    let Some(found) = available.iter().find(|t| t.to == target_status) else {
        return Ok(Outcome::NoSuchTransition { available: available.into_iter().map(|t| t.to).collect() });
    };
    let id = found.id.clone();
    let name = found.name.clone();
    provider.apply_transition(key, &id)?;
    Ok(Outcome::Applied { transition_name: name })
}
