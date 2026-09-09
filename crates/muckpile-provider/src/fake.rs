//! An in-memory provider for tests. The automated suite never talks to the
//! real provider — this is what it talks to instead. Hitting the real API
//! has its place, exploratory and by hand, never inside a suite that runs on
//! every `cargo test`.

use crate::provider::{Provider, Transition};
use anyhow::{bail, Result};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct FakeProvider {
    items: RefCell<HashMap<String, Item>>,
}

struct Item {
    status: String,
    transitions: Vec<Transition>,
}

impl FakeProvider {
    pub fn new() -> Self {
        FakeProvider { items: RefCell::new(HashMap::new()) }
    }

    /// Seeds an item at `status`, with `transitions` reachable from it.
    pub fn seed(&self, key: &str, status: &str, transitions: Vec<Transition>) {
        self.items.borrow_mut().insert(key.to_string(), Item { status: status.to_string(), transitions });
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
}
