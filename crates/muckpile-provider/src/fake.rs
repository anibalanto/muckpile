//! An in-memory provider for tests. The automated suite never talks to the
//! real provider — this is what it talks to instead. Hitting the real API
//! has its place, exploratory and by hand, never inside a suite that runs on
//! every `cargo test`.

use crate::provider::{self, Provider, Transition};
use anyhow::{bail, Result};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct FakeProvider {
    items: RefCell<HashMap<String, Entry>>,
}

/// What `FakeProvider` holds per item — a superset of what any one `Provider`
/// method needs, so `transition` and `item` tests can seed only what they use.
struct Entry {
    status: String,
    transitions: Vec<Transition>,
    jira_type: String,
    title: String,
    parent: Option<String>,
    body_adf: Option<String>,
}

impl FakeProvider {
    pub fn new() -> Self {
        FakeProvider { items: RefCell::new(HashMap::new()) }
    }

    /// Seeds an item at `status`, with `transitions` reachable from it.
    pub fn seed(&self, key: &str, status: &str, transitions: Vec<Transition>) {
        self.items.borrow_mut().insert(
            key.to_string(),
            Entry { status: status.to_string(), transitions, jira_type: String::new(), title: String::new(), parent: None, body_adf: None },
        );
    }

    /// Seeds an item with the fields `item()` reads.
    #[allow(clippy::too_many_arguments)]
    pub fn seed_item(&self, key: &str, jira_type: &str, title: &str, status: &str, parent: Option<&str>, body_adf: Option<&str>) {
        self.items.borrow_mut().insert(
            key.to_string(),
            Entry {
                status: status.to_string(),
                transitions: Vec::new(),
                jira_type: jira_type.to_string(),
                title: title.to_string(),
                parent: parent.map(|s| s.to_string()),
                body_adf: body_adf.map(|s| s.to_string()),
            },
        );
    }

    pub fn status_of(&self, key: &str) -> Option<String> {
        self.items.borrow().get(key).map(|i| i.status.clone())
    }
}

impl Default for FakeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for FakeProvider {
    fn transitions(&self, key: &str) -> Result<Vec<Transition>> {
        let items = self.items.borrow();
        let Some(item) = items.get(key) else { bail!("no such item: {key}") };
        Ok(item.transitions.clone())
    }

    fn apply_transition(&self, key: &str, transition_id: &str) -> Result<()> {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        let Some(t) = item.transitions.iter().find(|t| t.id == transition_id) else {
            bail!("no such transition: {transition_id}")
        };
        item.status = t.to.clone();
        Ok(())
    }

    fn item(&self, key: &str) -> Result<provider::Item> {
        let items = self.items.borrow();
        let Some(i) = items.get(key) else { bail!("no such item: {key}") };
        Ok(provider::Item {
            jira_type: i.jira_type.clone(),
            title: i.title.clone(),
            status: i.status.clone(),
            parent: i.parent.clone(),
            body_adf: i.body_adf.clone(),
        })
    }
}
