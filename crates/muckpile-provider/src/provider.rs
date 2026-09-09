//! The port every provider implements — real REST today, a future
//! muckpile-server tomorrow — so the rest of the system never has to notice
//! which one is behind it.

use anyhow::Result;

/// A workflow transition, exactly as the provider offers it.
///
/// The transition's name isn't the name of the status it leads to — on a
/// real board, the transition named "Listo" leaves an item in "Finalizada"
/// — so both fields exist: choosing one looks at `to`, applying it sends
/// `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub id: String,
    pub name: String,
    pub to: String,
}

pub trait Provider {
    /// The transitions the workflow allows **today** for this item. Asked
    /// fresh before every attempt, since it's where the id to apply comes
    /// from.
    fn transitions(&self, key: &str) -> Result<Vec<Transition>>;

    /// Applies one transition, by id.
    fn apply_transition(&self, key: &str, transition_id: &str) -> Result<()>;
}
