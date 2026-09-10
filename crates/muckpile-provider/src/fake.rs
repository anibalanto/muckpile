//! An in-memory provider for tests. The automated suite never talks to the
//! real provider — this is what it talks to instead. Hitting the real API
//! has its place, exploratory and by hand, never inside a suite that runs on
//! every `cargo test`.

use crate::provider::{self, Attachment, Comment, ItemLink, LinkType, Provider, Sprint, Status, Transition};
use anyhow::{bail, Context, Result};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};

pub struct FakeProvider {
    items: RefCell<HashMap<String, Entry>>,
    sprints: RefCell<Vec<Sprint>>,
    statuses: RefCell<Vec<Status>>,
    link_types: RefCell<Vec<LinkType>>,
    sprint_items: RefCell<HashMap<u64, Vec<String>>>,
    links_created: RefCell<Vec<(String, String, String)>>,
    next_keys: RefCell<VecDeque<(String, String)>>,
    next_id: RefCell<u64>,
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
    links: Vec<ItemLink>,
    labels: Vec<String>,
    comments: Vec<Comment>,
    attachments: Vec<(Attachment, Vec<u8>)>,
}

impl FakeProvider {
    pub fn new() -> Self {
        FakeProvider {
            items: RefCell::new(HashMap::new()),
            sprints: RefCell::new(Vec::new()),
            statuses: RefCell::new(Vec::new()),
            link_types: RefCell::new(Vec::new()),
            sprint_items: RefCell::new(HashMap::new()),
            links_created: RefCell::new(Vec::new()),
            next_keys: RefCell::new(VecDeque::new()),
            next_id: RefCell::new(1),
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
    /// which board id asks. Their ids are their positions, from 1.
    pub fn seed_sprints(&self, sprints: &[(&str, &str)]) {
        *self.sprints.borrow_mut() = sprints
            .iter()
            .enumerate()
            .map(|(i, (name, created))| Sprint { id: i as u64 + 1, name: name.to_string(), created: created.to_string() })
            .collect();
    }

    /// Seeds the keys a sprint holds.
    pub fn seed_sprint_items(&self, sprint_id: u64, keys: &[&str]) {
        self.sprint_items.borrow_mut().insert(sprint_id, keys.iter().map(|k| k.to_string()).collect());
    }

    /// Seeds an item at `status`, with `transitions` reachable from it.
    pub fn seed(&self, key: &str, status: &str, transitions: Vec<Transition>) {
        self.items.borrow_mut().insert(
            key.to_string(),
            Entry {
                status: status.to_string(),
                transitions,
                jira_type: String::new(),
                title: String::new(),
                parent: None,
                body_adf: None,
                links: Vec::new(),
                labels: Vec::new(),
                comments: Vec::new(),
                attachments: Vec::new(),
            },
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
                links: Vec::new(),
                labels: Vec::new(),
                comments: Vec::new(),
                attachments: Vec::new(),
            },
        );
    }

    /// Seeds the comments of an already-seeded item, oldest first.
    pub fn seed_comments(&self, key: &str, comments: Vec<Comment>) {
        let mut items = self.items.borrow_mut();
        items.get_mut(key).unwrap_or_else(|| panic!("FakeProvider: seed {key} before its comments")).comments = comments;
    }

    /// Attaches a file to an already-seeded item.
    pub fn seed_attachment(&self, key: &str, id: &str, filename: &str, bytes: &[u8]) {
        let mut items = self.items.borrow_mut();
        let item = items.get_mut(key).unwrap_or_else(|| panic!("FakeProvider: seed {key} before its attachments"));
        item.attachments.push((Attachment { id: id.to_string(), filename: filename.to_string() }, bytes.to_vec()));
    }

    /// Seeds the labels of an already-seeded item.
    pub fn seed_labels(&self, key: &str, labels: &[&str]) {
        let mut items = self.items.borrow_mut();
        let item = items.get_mut(key).unwrap_or_else(|| panic!("FakeProvider: seed {key} before its labels"));
        item.labels = labels.iter().map(|l| l.to_string()).collect();
    }

    pub fn labels_of(&self, key: &str) -> Vec<String> {
        self.items.borrow().get(key).map(|i| i.labels.clone()).unwrap_or_default()
    }

    /// Seeds the links an already-seeded item is on, as `(phrase, other)`
    /// pairs read from its own side — `("is blocked by", "ACC-340")`.
    pub fn seed_links(&self, key: &str, links: &[(&str, &str)]) {
        let mut items = self.items.borrow_mut();
        let item = items.get_mut(key).unwrap_or_else(|| panic!("FakeProvider: seed {key} before its links"));
        item.links = links.iter().map(|(phrase, other)| ItemLink { phrase: phrase.to_string(), other: other.to_string() }).collect();
    }

    /// An id for a comment or an attachment made here, never repeated.
    fn fresh_id(&self) -> String {
        let mut next = self.next_id.borrow_mut();
        *next += 1;
        format!("9{next:04}")
    }

    pub fn status_of(&self, key: &str) -> Option<String> {
        self.items.borrow().get(key).map(|i| i.status.clone())
    }

    pub fn title_of(&self, key: &str) -> Option<String> {
        self.items.borrow().get(key).map(|i| i.title.clone())
    }

    pub fn parent_of(&self, key: &str) -> Option<String> {
        self.items.borrow().get(key).and_then(|i| i.parent.clone())
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
            links: i.links.clone(),
            labels: i.labels.clone(),
            attachments: i.attachments.iter().map(|(a, _)| a.clone()).collect(),
        })
    }

    fn comments(&self, key: &str) -> Result<Vec<Comment>> {
        let items = self.items.borrow();
        let Some(i) = items.get(key) else { bail!("no such item: {key}") };
        Ok(i.comments.clone())
    }

    fn item_url(&self, key: &str) -> String {
        format!("https://fake.example/browse/{key}")
    }

    fn key_of_url(&self, url: &str) -> Option<String> {
        url.strip_prefix("https://fake.example/browse/").map(str::to_string)
    }

    fn types_of(&self, keys: &[String]) -> Result<BTreeMap<String, (String, Vec<String>)>> {
        let items = self.items.borrow();
        Ok(keys.iter().filter_map(|k| items.get(k).map(|i| (k.clone(), (i.jira_type.clone(), i.labels.clone())))).collect())
    }

    fn add_comment(&self, key: &str, body_adf: &str, parent: Option<&str>) -> Result<String> {
        let id = self.fresh_id();
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.comments.push(Comment {
            id: id.clone(),
            author: "fake".into(),
            author_id: "fake-id".into(),
            created: "2026-09-10T00:00:00.000-0300".into(),
            parent: parent.map(str::to_string),
            body_adf: body_adf.to_string(),
        });
        Ok(id)
    }

    fn add_attachment(&self, key: &str, filename: &str, bytes: &[u8]) -> Result<Attachment> {
        let attachment = Attachment { id: self.fresh_id(), filename: filename.to_string() };
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.attachments.push((attachment.clone(), bytes.to_vec()));
        Ok(attachment)
    }

    fn attachment_content(&self, attachment_id: &str) -> Result<Vec<u8>> {
        let items = self.items.borrow();
        items
            .values()
            .flat_map(|i| &i.attachments)
            .find(|(a, _)| a.id == attachment_id)
            .map(|(_, bytes)| bytes.clone())
            .with_context(|| format!("no such attachment: {attachment_id}"))
    }

    fn sprint_items(&self, sprint_id: u64) -> Result<Vec<String>> {
        Ok(self.sprint_items.borrow().get(&sprint_id).cloned().unwrap_or_default())
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

    /// Records the link, and — when the type was seeded — puts it on both
    /// items it joins, each told from its own side, the way `item` reads a
    /// link back.
    fn create_link(&self, type_name: &str, outward_key: &str, inward_key: &str) -> Result<()> {
        self.links_created.borrow_mut().push((type_name.to_string(), outward_key.to_string(), inward_key.to_string()));
        let Some(t) = self.link_types.borrow().iter().find(|t| t.name == type_name).cloned() else { return Ok(()) };
        let mut items = self.items.borrow_mut();
        if let Some(item) = items.get_mut(outward_key) {
            item.links.push(ItemLink { phrase: t.outward.clone(), other: inward_key.to_string() });
        }
        if let Some(item) = items.get_mut(inward_key) {
            item.links.push(ItemLink { phrase: t.inward.clone(), other: outward_key.to_string() });
        }
        Ok(())
    }

    /// Undoes `create_link`: the link leaves `links_created` and, when the
    /// type was seeded, both items it joined. A link is here only when
    /// `create_link` made it, in this same direction.
    fn delete_link(&self, type_name: &str, outward_key: &str, inward_key: &str) -> Result<bool> {
        let mut created = self.links_created.borrow_mut();
        let Some(at) = created.iter().position(|(t, o, i)| t == type_name && o == outward_key && i == inward_key) else { return Ok(false) };
        created.remove(at);
        let Some(t) = self.link_types.borrow().iter().find(|t| t.name == type_name).cloned() else { return Ok(true) };
        let mut items = self.items.borrow_mut();
        let mut drop_one = |key: &str, phrase: &str, other: &str| {
            if let Some(item) = items.get_mut(key) {
                if let Some(at) = item.links.iter().position(|l| l.phrase == phrase && l.other == other) {
                    item.links.remove(at);
                }
            }
        };
        drop_one(outward_key, &t.outward, inward_key);
        drop_one(inward_key, &t.inward, outward_key);
        Ok(true)
    }

    fn update_title(&self, key: &str, title: &str) -> Result<()> {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.title = title.to_string();
        Ok(())
    }

    fn set_parent(&self, key: &str, parent_key: &str) -> Result<()> {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.parent = Some(parent_key.to_string());
        Ok(())
    }

    fn update_body(&self, key: &str, body_adf: &str) -> Result<()> {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.get_mut(key) else { bail!("no such item: {key}") };
        item.body_adf = Some(body_adf.to_string());
        Ok(())
    }

    fn find_by_title(&self, _project_key: &str, jira_type: &str, label: Option<&str>, title: &str) -> Result<Option<String>> {
        let carries = |e: &Entry| label.is_none_or(|l| e.labels.iter().any(|have| have == l));
        Ok(self.items.borrow().iter().find(|(_, e)| e.jira_type == jira_type && e.title == title && carries(e)).map(|(k, _)| k.clone()))
    }

    fn create_item(
        &self,
        _project_key: &str,
        jira_type: &str,
        label: Option<&str>,
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
                links: Vec::new(),
                labels: label.map(|l| vec![l.to_string()]).unwrap_or_default(),
                comments: Vec::new(),
                attachments: Vec::new(),
            },
        );
        Ok(key)
    }
}
