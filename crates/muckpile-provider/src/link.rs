//! Deciding which of the provider's own relationship types matches a
//! requested phrase, and firing it in the right direction. No vocabulary of
//! muckpile's own — a phrase is either a type's `outward` or `inward`
//! wording, verbatim, the same idea `transition` already applies to status.

use crate::provider::{LinkType, Provider};
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
/// The phrase can come with `_` for each space, the way an item's header
/// shows it: `is_blocked_by` is `is blocked by`.
pub fn link(provider: &dyn Provider, a: &str, phrase: &str, b: &str) -> Result<Outcome> {
    let types = provider.link_types()?;
    let Some(edge) = edge(&types, a, phrase, b) else {
        return Ok(Outcome::NoSuchPhrase { available: phrases(&types) });
    };
    provider.create_link(&edge.type_name, edge.outward_key, edge.inward_key)?;
    Ok(Outcome::Applied { type_name: edge.type_name })
}

pub enum UnlinkOutcome {
    /// The link was removed. `type_name` is the provider's name for the
    /// relationship, as in `Outcome::Applied`.
    Removed { type_name: String },
    /// No type the provider offers uses this phrase, from either direction.
    NoSuchPhrase { available: Vec<String> },
    /// The phrase names a `type_name` edge the provider doesn't have — not
    /// a transport error, and nothing was removed.
    NoSuchLink { type_name: String },
}

/// Removes the link `link` with the same `a phrase b` creates: the same
/// type, the same direction, the same two ways to write the phrase. When the
/// type says the same both ways — `Relates` — the phrase can't tell which
/// end the link was made from, so the other direction is tried too.
pub fn unlink(provider: &dyn Provider, a: &str, phrase: &str, b: &str) -> Result<UnlinkOutcome> {
    let types = provider.link_types()?;
    let Some(edge) = edge(&types, a, phrase, b) else {
        return Ok(UnlinkOutcome::NoSuchPhrase { available: phrases(&types) });
    };
    let removed = provider.delete_link(&edge.type_name, edge.outward_key, edge.inward_key)?
        || (edge.symmetric && provider.delete_link(&edge.type_name, edge.inward_key, edge.outward_key)?);
    if removed {
        Ok(UnlinkOutcome::Removed { type_name: edge.type_name })
    } else {
        Ok(UnlinkOutcome::NoSuchLink { type_name: edge.type_name })
    }
}

/// The edge `a phrase b` names, in the provider's terms: which type, and
/// which of the two keys plays its outward phrase.
struct Edge<'k> {
    type_name: String,
    outward_key: &'k str,
    inward_key: &'k str,
    /// The type says the same both ways, so the phrase alone doesn't say
    /// which key plays which end.
    symmetric: bool,
}

/// The first type among `types` whose outward or inward wording is
/// `phrase` — verbatim, or with `_` for each space — and the edge
/// `a phrase b` is of it. `None` when no type says `phrase`.
fn edge<'k>(types: &[LinkType], a: &'k str, phrase: &str, b: &'k str) -> Option<Edge<'k>> {
    let says = |wording: &str| wording == phrase || wording.replace(' ', "_") == phrase;
    types.iter().find_map(|t| {
        let symmetric = t.outward == t.inward;
        if says(&t.outward) {
            Some(Edge { type_name: t.name.clone(), outward_key: a, inward_key: b, symmetric })
        } else if says(&t.inward) {
            Some(Edge { type_name: t.name.clone(), outward_key: b, inward_key: a, symmetric })
        } else {
            None
        }
    })
}

/// Every phrase `types` offer, from either direction, once each and sorted.
fn phrases(types: &[LinkType]) -> Vec<String> {
    let available: BTreeSet<String> = types.iter().flat_map(|t| [t.outward.clone(), t.inward.clone()]).collect();
    available.into_iter().collect()
}
