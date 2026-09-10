//! The real transport: Jira's REST API, reached directly — one port, not
//! three transports each covering for what the other two can't do.

use crate::jql::search_text;
use crate::provider::{Item, ItemLink, LinkType, Provider, Sprint, Status, Transition};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;

/// What proves the request is this account. Never written to disk as a
/// whole — only the token's environment variable name is, and the value is
/// read from the environment at the moment it's needed.
pub struct Credentials {
    email: String,
    token: String,
}

impl Credentials {
    pub fn new(email: impl Into<String>, token: impl Into<String>) -> Self {
        Credentials { email: email.into(), token: token.into() }
    }

    fn basic_auth(&self) -> String {
        format!("Basic {}", base64(format!("{}:{}", self.email, self.token).as_bytes()))
    }
}

/// A Jira Cloud instance, reached over its REST API.
pub struct JiraRest {
    base: String,
    creds: Credentials,
}

impl JiraRest {
    pub fn new(base: impl Into<String>, creds: Credentials) -> Self {
        JiraRest { base: base.into().trim_end_matches('/').to_string(), creds }
    }

    fn call(&self, method: &str, path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let url = format!("{}{path}", self.base);
        let (status, text) =
            http(method, &url, &self.creds.basic_auth(), body).with_context(|| format!("talking to {url}"))?;
        if !(200..300).contains(&status) {
            bail!("{method} {path} returned {status}: {text}");
        }
        if text.trim().is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&text)
            .with_context(|| format!("{method} {path} returned {status} and it wasn't JSON: {text}"))
    }
}

/// One entry of an issue's `issuelinks`, read from that issue's side. The
/// entry names only the other end: `outwardIssue` when this issue plays the
/// type's outward phrase toward it (`blocks`), `inwardIssue` when it plays
/// the inward one (`is blocked by`) — measured on ACC, where the same link
/// reads `outwardIssue: ACC-338` on ACC-340 and `inwardIssue: ACC-340` on
/// ACC-338.
fn link_from_own_side(entry: &serde_json::Value) -> Option<ItemLink> {
    let link_type = entry.get("type")?;
    let (phrase, other) = match (entry.get("outwardIssue"), entry.get("inwardIssue")) {
        (Some(other), _) => (link_type.get("outward")?, other),
        (None, Some(other)) => (link_type.get("inward")?, other),
        (None, None) => return None,
    };
    Some(ItemLink { phrase: phrase.as_str()?.to_string(), other: other.get("key")?.as_str()?.to_string() })
}

impl Provider for JiraRest {
    fn transitions(&self, key: &str) -> Result<Vec<Transition>> {
        let v = self.call("GET", &format!("/rest/api/3/issue/{key}/transitions"), None)?;
        let arr = v
            .get("transitions")
            .and_then(|t| t.as_array())
            .ok_or_else(|| anyhow!("the response for {key} carries no `transitions`"))?;
        Ok(arr
            .iter()
            .filter_map(|t| {
                Some(Transition {
                    id: t.get("id")?.as_str()?.to_string(),
                    name: t.get("name")?.as_str().unwrap_or_default().to_string(),
                    to: t.get("to")?.get("name")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    fn apply_transition(&self, key: &str, transition_id: &str) -> Result<()> {
        let body = serde_json::json!({ "transition": { "id": transition_id } });
        self.call("POST", &format!("/rest/api/3/issue/{key}/transitions"), Some(body))?;
        Ok(())
    }

    fn item(&self, key: &str) -> Result<Item> {
        let v = self.call("GET", &format!("/rest/api/3/issue/{key}?fields=summary,status,issuetype,parent,description,issuelinks"), None)?;
        let fields = v.get("fields").ok_or_else(|| anyhow!("the response for {key} carries no `fields`"))?;
        let title = fields.get("summary").and_then(|s| s.as_str()).unwrap_or_default().to_string();
        let status = fields
            .get("status")
            .and_then(|s| s.get("name"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("{key}: no fields.status.name in the response"))?
            .to_string();
        let jira_type = fields
            .get("issuetype")
            .and_then(|s| s.get("name"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("{key}: no fields.issuetype.name in the response"))?
            .to_string();
        let parent = fields.get("parent").and_then(|p| p.get("key")).and_then(|k| k.as_str()).map(str::to_string);
        let body_adf = fields.get("description").filter(|d| !d.is_null()).map(|d| d.to_string());
        let links = fields.get("issuelinks").and_then(|l| l.as_array()).map(|l| l.iter().filter_map(link_from_own_side).collect()).unwrap_or_default();
        Ok(Item { jira_type, title, status, parent, body_adf, links })
    }

    fn open_sprints(&self, board_id: u64) -> Result<Vec<Sprint>> {
        let v = self.call("GET", &format!("/rest/agile/1.0/board/{board_id}/sprint?state=active"), None)?;
        let arr = v.get("values").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("no `values` in the sprint response"))?;
        Ok(arr
            .iter()
            .filter_map(|s| {
                Some(Sprint {
                    name: s.get("name")?.as_str()?.to_string(),
                    created: s.get("createdDate")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    fn project_statuses(&self, project_key: &str) -> Result<Vec<Status>> {
        let v = self.call("GET", &format!("/rest/api/3/project/{project_key}/statuses"), None)?;
        let issue_types = v.as_array().ok_or_else(|| anyhow!("the response for {project_key} isn't a list of issue types"))?;

        // A status is shared verbatim by every issue type that offers it —
        // measured against ACC/701 — so a map keyed by name, last write
        // wins, is exactly the dedup this needs.
        let mut by_name: BTreeMap<String, String> = BTreeMap::new();
        for issue_type in issue_types {
            let Some(statuses) = issue_type.get("statuses").and_then(|s| s.as_array()) else { continue };
            for s in statuses {
                let (Some(name), Some(category)) = (
                    s.get("name").and_then(|n| n.as_str()),
                    s.get("statusCategory").and_then(|c| c.get("key")).and_then(|k| k.as_str()),
                ) else {
                    continue;
                };
                by_name.insert(name.to_string(), category.to_string());
            }
        }
        Ok(by_name.into_iter().map(|(name, category)| Status { name, category }).collect())
    }

    fn link_types(&self) -> Result<Vec<LinkType>> {
        let v = self.call("GET", "/rest/api/3/issueLinkType", None)?;
        let arr = v.get("issueLinkTypes").and_then(|t| t.as_array()).ok_or_else(|| anyhow!("no `issueLinkTypes` in the response"))?;
        Ok(arr
            .iter()
            .filter_map(|t| {
                Some(LinkType {
                    name: t.get("name")?.as_str()?.to_string(),
                    outward: t.get("outward")?.as_str()?.to_string(),
                    inward: t.get("inward")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    fn create_link(&self, type_name: &str, outward_key: &str, inward_key: &str) -> Result<()> {
        // The fields name the ends of the link object, not the phrase each
        // issue says: the one that "blocks" is the link's inward issue —
        // measured, a link posted the other way round comes back reversed.
        let body = serde_json::json!({
            "type": { "name": type_name },
            "inwardIssue": { "key": outward_key },
            "outwardIssue": { "key": inward_key },
        });
        self.call("POST", "/rest/api/3/issueLink", Some(body))?;
        Ok(())
    }

    fn update_title(&self, key: &str, title: &str) -> Result<()> {
        let body = serde_json::json!({ "fields": { "summary": title } });
        self.call("PUT", &format!("/rest/api/3/issue/{key}"), Some(body))?;
        Ok(())
    }

    fn update_body(&self, key: &str, body_adf: &str) -> Result<()> {
        let adf: serde_json::Value =
            serde_json::from_str(body_adf).with_context(|| format!("{key}: the body to send isn't JSON"))?;
        let body = serde_json::json!({ "fields": { "description": adf } });
        self.call("PUT", &format!("/rest/api/3/issue/{key}"), Some(body))?;
        Ok(())
    }

    fn find_by_title(&self, project_key: &str, jira_type: &str, title: &str) -> Result<Option<String>> {
        let needle = search_text(title);
        if needle.is_empty() {
            bail!("{title:?}: no queda nada con qué buscar después de reducirlo para JQL");
        }
        let jql = format!("project = {project_key} AND issuetype = \"{jira_type}\" AND summary ~ \"{needle}\"");
        let path = format!("/rest/api/3/search?jql={}&fields=summary", url_encode(&jql));
        let v = self.call("GET", &path, None)?;
        let issues = v.get("issues").and_then(|i| i.as_array()).ok_or_else(|| anyhow!("no `issues` in the search response"))?;
        // The JQL is deliberately imprecise (`~` is full-text, not literal):
        // the exact match is decided here, by comparing the whole `summary`.
        Ok(issues.iter().find_map(|issue| {
            let summary = issue.get("fields")?.get("summary")?.as_str()?;
            if summary != title {
                return None;
            }
            issue.get("key")?.as_str().map(str::to_string)
        }))
    }

    fn create_item(&self, project_key: &str, jira_type: &str, title: &str, parent: Option<&str>, body_adf: Option<&str>) -> Result<String> {
        let mut fields = serde_json::json!({
            "project": { "key": project_key },
            "issuetype": { "name": jira_type },
            "summary": title,
        });
        if let Some(p) = parent {
            fields["parent"] = serde_json::json!({ "key": p });
        }
        if let Some(adf) = body_adf {
            let adf_value: serde_json::Value = serde_json::from_str(adf).context("the body to send isn't JSON")?;
            fields["description"] = adf_value;
        }
        let v = self.call("POST", "/rest/api/3/issue", Some(serde_json::json!({ "fields": fields })))?;
        v.get("key")
            .and_then(|k| k.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("create no devolvió 'key': {v}"))
    }
}

/// The only thing that touches `ureq`. A `4xx`/`5xx` isn't an `Err` here —
/// it's a response with a status and a body, and the body is where Jira says
/// which field it didn't like. `Err` is only for what kept the question from
/// being asked at all — DNS, TLS, the connection.
fn http(method: &str, url: &str, auth: &str, body: Option<serde_json::Value>) -> Result<(u16, String)> {
    let req = ureq::request(method, url).set("Authorization", auth).set("Accept", "application/json");
    let res = match body {
        Some(b) => req.set("Content-Type", "application/json").send_string(&b.to_string()),
        None => req.call(),
    };
    match res {
        Ok(r) => {
            let status = r.status();
            Ok((status, r.into_string().unwrap_or_default()))
        }
        Err(ureq::Error::Status(status, r)) => Ok((status, r.into_string().unwrap_or_default())),
        Err(e) => Err(anyhow!("{e}")),
    }
}

/// Percent-encodes `s` for a URL query value — byte by byte, so a
/// multi-byte UTF-8 character (an accent in a title) comes out as one
/// `%XX` per byte, which is exactly what a percent-decoder expects back.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i)) as usize & 0x3f] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
