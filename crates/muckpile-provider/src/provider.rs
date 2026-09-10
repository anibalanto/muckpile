//! The port every provider implements — real REST today, a future
//! muckpile-server tomorrow — so the rest of the system never has to notice
//! which one is behind it.

use anyhow::Result;
use std::collections::BTreeMap;

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
    /// Every link the item is on, each read from the item's own side.
    pub links: Vec<ItemLink>,
    /// The item's labels — what tells apart two muckpile types that share
    /// one provider type.
    pub labels: Vec<String>,
    /// The files attached to the item.
    pub attachments: Vec<Attachment>,
}

/// One comment on an item, as the provider holds it. `author` is the name a
/// person reads and isn't unique; `author_id` is the account, which is.
/// `parent` is the comment this one replies to — `None` for a root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub author_id: String,
    pub created: String,
    pub parent: Option<String>,
    /// The comment's body, as ADF, serialized.
    pub body_adf: String,
}

/// One file attached to an item: its id, and the name it was uploaded with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
}

/// One link, seen from the item that carries it: the phrase from its own
/// side, and the item at the other end. A `Blocks` link between ACC-340 and
/// ACC-338 is `blocks ACC-338` on ACC-340, and `is blocked by ACC-340` on
/// ACC-338 — the same edge, told from each end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLink {
    pub phrase: String,
    pub other: String,
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

    /// Every comment on the item, oldest first, each with the comment it
    /// replies to.
    fn comments(&self, key: &str) -> Result<Vec<Comment>>;

    /// The bytes of one attachment.
    fn attachment_content(&self, attachment_id: &str) -> Result<Vec<u8>>;

    /// Adds a comment, as ADF — a reply to `parent` when there is one — and
    /// returns its id.
    fn add_comment(&self, key: &str, body_adf: &str, parent: Option<&str>) -> Result<String>;

    /// Attaches a file to the item, under `filename`.
    fn add_attachment(&self, key: &str, filename: &str, bytes: &[u8]) -> Result<Attachment>;

    /// The page of an item on the provider — what a card to it points at.
    fn item_url(&self, key: &str) -> String;

    /// The item a URL is the page of, when it is one of this provider's.
    fn key_of_url(&self, url: &str) -> Option<String>;

    /// Each key's provider type and labels, in one question — a key the
    /// provider doesn't have is simply missing from the answer.
    fn types_of(&self, keys: &[String]) -> Result<BTreeMap<String, (String, Vec<String>)>>;

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

    /// Makes `parent_key` the item's parent, replacing whichever it had.
    /// Whether that parent exists, and may hold an item of this type, is the
    /// provider's to check.
    fn set_parent(&self, key: &str, parent_key: &str) -> Result<()>;

    /// Overwrites the item's body, as ADF. Only ever called with a body that
    /// round-trips losslessly through this same conversion, so what lands
    /// here is always something this system can read back exactly.
    fn update_body(&self, key: &str, body_adf: &str) -> Result<()>;

    /// The key of the item whose title is exactly `title`, among those of
    /// `jira_type` in `project_key` — carrying `label`, when there is one —
    /// `None` when there's no such item. Checked before creating, so a retry
    /// after a partial failure doesn't duplicate what an earlier attempt
    /// already made.
    fn find_by_title(&self, project_key: &str, jira_type: &str, label: Option<&str>, title: &str) -> Result<Option<String>>;

    /// Creates a new item and returns its key. `parent` and `body_adf`
    /// travel only here — an item this finds instead of creates never gets
    /// either applied after the fact, because there was nothing new to
    /// write in the first place.
    fn create_item(
        &self,
        project_key: &str,
        jira_type: &str,
        label: Option<&str>,
        title: &str,
        parent: Option<&str>,
        body_adf: Option<&str>,
    ) -> Result<String>;
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
