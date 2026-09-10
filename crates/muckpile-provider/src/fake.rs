//! An in-memory provider for tests. The automated suite never talks to the
//! real provider — this is what it talks to instead. Hitting the real API
//! has its place, exploratory and by hand, never inside a suite that runs on
//! every `cargo test`.

use crate::provider::{self, LinkType, Provider, Sprint, Status, Transition};
use anyhow::{bail, Context, Result};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

pub struct FakeProvider {
    items: RefCell<HashMap<String, Entry>>,
    sprints: RefCell<Vec<Sprint>>,
    statuses: RefCell<Vec<Status>>,
    link_types: RefCell<Vec<LinkType>>,
    links_created: RefCell<Vec<(String, String, String)>>,
    next_keys: RefCell<VecDeque<(String, String)>>,
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
        FakeProvider {
            items: RefCell::new(HashMap::new()),
            sprints: RefCell::new(Vec::new()),
            statuses: RefCell::new(Vec::new()),
            link_types: RefCell::new(Vec::new()),
            links_created: RefCell::new(Vec::new()),
            next_keys: RefCell::new(VecDeque::new()),
        }
    }

    /// Queues the `(key, initial status)` `create_item` hands back the next
    /// time it's called — in order, one pair consumed per call. A test
    /// decides the key and status explicitly instead of this guessing a
    /// workflow's real default.
    pub fn queue_create(&self, key: &str, initial_status: &str) {
        self.next_keys.borrow_mut().push_back((key.to_string(), initial_status.to_string()));
    }

    /// Seeds the provider's own relationship types as `(name, outward,
    /// inward)` triples.
    pub fn seed_link_types(&self, types: &[(&str, &str, &str)]) {
        *self.link_types.borrow_mut() = types
            .iter()
            .map(|(name, outward, inward)| LinkType { name: name.to_string(), outward: outward.to_string(), inward: inward.to_string() })
            .collect();
    }

    /// Every link `create_link` was asked to make, as `(type, outward_key,
    /// inward_key)` — what a test checks instead of a live board.
    pub fn links_created(&self) -> Vec<(String, String, String)> {
        self.links_created.borrow().clone()
    }

    /// Seeds the project's workflow statuses as `(name, category)` pairs,
    /// ignoring which project key asks.
    pub fn seed_statuses(&self, statuses: &[(&str, &str)]) {
        *self.statuses.borrow_mut() =
            statuses.iter().map(|(name, category)| Status { name: name.to_string(), category: category.to_string() }).collect();
    }

    /// Seeds the board's open sprints as `(name, created)` pairs, ignoring
    /// which board id asks.
    pub fn seed_sprints(&self, sprints: &[(&str, &str)]) {
        *self.sprints.borrow_mut() =
            sprints.iter().map(|(name, created)| Sprint { name: name.to_string(), created: created.to_string() }).collect();
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

    pub fn title_of(&self, key: &str) -> Option<String> {
        self.items.borrow().get(key).map(|i| i.title.clone())
    }

    pub fn body_adf_of(&self, key: &str) -> Option<String> {
        self.items.borrow().get(key).and_then(|i| i.body_adf.clone())
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

    fn open_sprints(&self, _board_id: u64) -> Result<Vec<Sprint>> {
        Ok(self.sprints.borrow().clone())
    }

    fn project_statuses(&self, _project_key: &str) -> Result<Vec<Status>> {
        Ok(self.statuses.borrow().clone())
    }

    fn link_types(&self) -> Result<Vec<LinkType>> {
        Ok(self.link_types.borrow().clone())
    }

    fn create_link(&self, type_name: &str, outward_key: &str, inward_key: &str) -> Result<()> {
        self.links_created.borrow_mut().push((type_name.to_string(), outward_key.to_string(), inward_key.to_string()));
        Ok(())
    }

    fn update_title(&self, key: &str, title: &str) -> Result<()> {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.title = title.to_string();
        Ok(())
    }

    fn update_body(&self, key: &str, body_adf: &str) -> Result<()> {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.body_adf = Some(body_adf.to_string());
        Ok(())
    }

    fn find_by_title(&self, _project_key: &str, jira_type: &str, title: &str) -> Result<Option<String>> {
        Ok(self.items.borrow().iter().find(|(_, e)| e.jira_type == jira_type && e.title == title).map(|(k, _)| k.clone()))
    }

    fn create_item(
        &self,
        _project_key: &str,
        jira_type: &str,
        title: &str,
        parent: Option<&str>,
        body_adf: Option<&str>,
    ) -> Result<String> {
        let (key, status) = self
            .next_keys
            .borrow_mut()
            .pop_front()
            .with_context(|| "FakeProvider: no queued key — call queue_create first")?;
        self.items.borrow_mut().insert(
            key.clone(),
            Entry {
                status,
                transitions: Vec::new(),
                jira_type: jira_type.to_string(),
                title: title.to_string(),
                parent: parent.map(str::to_string),
                body_adf: body_adf.map(str::to_string),
            },
        );
        Ok(key)
    }
}
