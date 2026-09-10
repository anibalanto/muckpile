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

/// An item's fields, exactly as the provider holds them — no translation:
/// `status` and `jira_type` are the provider's own strings (decision 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub jira_type: String,
    pub title: String,
    pub status: String,
    pub parent: Option<String>,
    /// The description, as ADF, serialized. `None` when the item has none.
    pub body_adf: Option<String>,
}

pub trait Provider {
    /// The transitions the workflow allows **today** for this item. Asked
    /// fresh before every attempt, since it's where the id to apply comes
    /// from.
    fn transitions(&self, key: &str) -> Result<Vec<Transition>>;

    /// Applies one transition, by id.
    fn apply_transition(&self, key: &str, transition_id: &str) -> Result<()>;

    /// The item's current fields.
    fn item(&self, key: &str) -> Result<Item>;

    /// The board's currently open sprints.
    fn open_sprints(&self, board_id: u64) -> Result<Vec<Sprint>>;

    /// Every status the project's workflow uses today, one per name — a
    /// status can be offered by more than one issue type, but its category
    /// never differs between them on the same project (measured against
    /// ACC/701: three statuses, shared verbatim across six issue types).
    fn project_statuses(&self, project_key: &str) -> Result<Vec<Status>>;

    /// Every relationship type the provider offers, instance-wide — measured
    /// against the real Jira instance behind ACC: eleven types, none named
    /// `Depends`, so `depends`/`blocks` was never going to be muckpile's own
    /// vocabulary to keep (decision 8 extended from status to this).
    fn link_types(&self) -> Result<Vec<LinkType>>;

    /// Creates one link of `type_name` — `outward_key`'s issue plays that
    /// type's outward phrase toward `inward_key`'s (`outward_key` "blocks"
    /// `inward_key`, for `Blocks`).
    fn create_link(&self, type_name: &str, outward_key: &str, inward_key: &str) -> Result<()>;

    /// Overwrites the item's title — the `summary` field — with exactly what
    /// was written locally, no conversion.
    fn update_title(&self, key: &str, title: &str) -> Result<()>;

    /// Overwrites the item's body, as ADF. Only ever called with a body that
    /// round-trips losslessly through this same conversion, so what lands
    /// here is always something this system can read back exactly.
    fn update_body(&self, key: &str, body_adf: &str) -> Result<()>;
}

/// A relationship type, named from both directions — e.g. Jira's `Blocks`:
/// outward `"blocks"`, inward `"is blocked by"`. Neither phrase is
/// muckpile's own: both are exactly what the provider calls them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkType {
    pub name: String,
    pub outward: String,
    pub inward: String,
}

/// A workflow status, as the provider names and categorizes it. `category`
/// is the provider's own key (`new`/`indeterminate`/`done` on Jira) — fixed
/// and independent of `name`'s language, unlike `name` itself (decision 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub name: String,
    pub category: String,
}

/// A sprint, as the provider names and dates it — a project's own naming
/// convention (`"22 Las vistas"`) is the provider's data, not something to
/// invent an id for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprint {
    pub name: String,
    /// When the provider created the sprint, ISO-8601. Read only when
    /// `name` is provider-truncated — see `sprint_fetch`.
    pub created: String,
}
