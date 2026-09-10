//! Deciding which of the provider's own relationship types matches a
//! requested phrase, and firing it in the right direction. No vocabulary of
//! muckpile's own — a phrase is either a type's `outward` or `inward`
//! wording, verbatim, the same idea `transition` already applies to status
//! (decision 8).

use crate::provider::Provider;
use anyhow::Result;
use std::collections::BTreeSet;

pub enum Outcome {
    /// The link was created. `type_name` is the provider's name for the
    /// relationship, which isn't necessarily either phrase asked for.
    Applied { type_name: String },
    /// No type the provider offers uses this phrase, from either direction.
    /// Not a transport error — the answer, with what the provider does offer.
    NoSuchPhrase { available: Vec<String> },
}

/// `a phrase b`: if `phrase` is a type's outward wording, `a` plays that
/// type's outward role toward `b`; if it's the inward wording, the roles
/// reverse — `"b is blocked by a"` and `"a blocks b"` are the same edge.
pub fn link(provider: &dyn Provider, a: &str, phrase: &str, b: &str) -> Result<Outcome> {
    let types = provider.link_types()?;
    for t in &types {
        if t.outward == phrase {
            provider.create_link(&t.name, a, b)?;
            return Ok(Outcome::Applied { type_name: t.name.clone() });
        }
        if t.inward == phrase {
            provider.create_link(&t.name, b, a)?;
            return Ok(Outcome::Applied { type_name: t.name.clone() });
        }
    }
    let available: BTreeSet<String> = types.iter().flat_map(|t| [t.outward.clone(), t.inward.clone()]).collect();
    Ok(Outcome::NoSuchPhrase { available: available.into_iter().collect() })
}
