//! Verifies `JiraRest` builds the right request — method, path, auth header,
//! body — against a local, in-process mock. Never touches the real
//! provider: this is exactly what the automated suite is allowed to touch.

use muckpile_provider::provider::{Attachment, Comment, ItemLink, LinkType, Provider, Sprint, Status, Transition};
use muckpile_provider::rest::{Credentials, JiraRest};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

struct Captured {
    method: String,
    path: String,
    body: String,
    raw: String,
}

/// A server that answers exactly one request with `response_body` at
/// `status`, then hands back what it received.
fn one_shot(status: u16, response_body: &str) -> (String, mpsc::Receiver<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let response_body = response_body.to_string();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        // ureq writes headers and body as separate socket writes, so a
        // single `read` can land between the two: keep reading until the
        // headers are in and the body is as long as `Content-Length` says.
        let mut data: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let text = String::from_utf8_lossy(&data);
            if let Some(header_end) = text.find("\r\n\r\n") {
                let content_length: usize = text[..header_end]
                    .lines()
                    .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("content-length")))
                    .and_then(|(_, v)| v.trim().parse().ok())
                    .unwrap_or(0);
                if data.len() - (header_end + 4) >= content_length {
                    break;
                }
            }
            let n = stream.read(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            data.extend_from_slice(&chunk[..n]);
        }
        let text = String::from_utf8_lossy(&data).to_string();
        let mut lines = text.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split(' ');
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
        tx.send(Captured { method, path, body, raw: text }).unwrap();

        let resp = format!(
            "HTTP/1.1 {status} x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(resp.as_bytes()).unwrap();
    });
    (format!("http://{addr}"), rx)
}

#[test]
fn transitions_hits_the_transitions_endpoint_with_get() {
    let response = r#"{"transitions":[{"id":"31","name":"Listo","to":{"name":"Finalizada"}}]}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let transitions = provider.transitions("ACC-355").unwrap();
    assert_eq!(transitions, vec![Transition { id: "31".into(), name: "Listo".into(), to: "Finalizada".into() }]);

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "GET");
    assert_eq!(captured.path, "/rest/api/3/issue/ACC-355/transitions");
}

#[test]
fn apply_transition_posts_the_id_never_the_name() {
    let (base, rx) = one_shot(204, "");
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    provider.apply_transition("ACC-355", "31").unwrap();

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/rest/api/3/issue/ACC-355/transitions");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["transition"]["id"], "31");
    assert!(captured.body.contains("31") && !captured.body.contains("Listo"), "the name never travels: {}", captured.body);
}

/// `base64("a@b.com:tok")`, computed once and hardcoded, so the test doesn't
/// re-derive it from the same encoder it's checking.
#[test]
fn credentials_travel_as_basic_auth() {
    let (base, rx) = one_shot(200, r#"{"transitions":[]}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));
    provider.transitions("ACC-1").unwrap();

    let captured = rx.recv().unwrap();
    assert!(
        captured.raw.contains("Authorization: Basic YUBiLmNvbTp0b2s="),
        "missing or wrong auth header: {}",
        captured.raw
    );
}

#[test]
fn a_rejected_status_surfaces_the_response_body() {
    let (base, rx) = one_shot(400, r#"{"errorMessages":["field X is invalid"]}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let err = provider.transitions("ACC-1").unwrap_err();
    assert!(format!("{err:#}").contains("field X is invalid"), "{err:#}");
    rx.recv().unwrap();
}

#[test]
fn item_hits_the_issue_endpoint_and_parses_every_field() {
    let response = r#"{"fields":{
        "summary":"Vistas de trabajo",
        "status":{"name":"En curso"},
        "issuetype":{"name":"Tarea"},
        "parent":{"key":"ACC-100"},
        "description":{"type":"doc","version":1,"content":[]}
    }}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let item = provider.item("ACC-355").unwrap();
    assert_eq!(item.title, "Vistas de trabajo");
    assert_eq!(item.status, "En curso");
    assert_eq!(item.jira_type, "Tarea");
    assert_eq!(item.parent.as_deref(), Some("ACC-100"));
    assert!(item.body_adf.is_some());

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "GET");
    assert!(captured.path.starts_with("/rest/api/3/issue/ACC-355"), "{}", captured.path);
}

/// Shape measured against ACC on 2026-09-10: on ACC-338, the link that
/// ACC-340 blocks comes as `inwardIssue: ACC-340` — read from ACC-338's side,
/// "is blocked by ACC-340". On ACC-340 the same link comes as
/// `outwardIssue: ACC-338`, "blocks ACC-338".
#[test]
fn item_reads_each_link_from_the_item_s_own_side() {
    let response = r#"{"fields":{
        "summary":"x","status":{"name":"En curso"},"issuetype":{"name":"Tarea"},
        "issuelinks":[
          {"id":"54926","type":{"name":"Blocks","inward":"is blocked by","outward":"blocks"},"inwardIssue":{"key":"ACC-340"}},
          {"id":"55080","type":{"name":"Blocks","inward":"is blocked by","outward":"blocks"},"outwardIssue":{"key":"ACC-335"}}
        ]
    }}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let item = provider.item("ACC-338").unwrap();
    assert_eq!(
        item.links,
        vec![
            ItemLink { phrase: "is blocked by".into(), other: "ACC-340".into() },
            ItemLink { phrase: "blocks".into(), other: "ACC-335".into() },
        ]
    );
    let captured = rx.recv().unwrap();
    assert!(captured.path.contains("issuelinks"), "{}", captured.path);
}

#[test]
fn open_sprints_hits_the_board_s_sprint_endpoint_and_lists_names_and_dates_verbatim() {
    let response = r#"{"values":[
        {"id":1,"name":"22 Las vistas","state":"active","createdDate":"2026-08-01T00:00:00.000Z"},
        {"id":2,"name":"23 Las questions","state":"active","createdDate":"2026-08-05T00:00:00.000Z"}
    ]}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let sprints = provider.open_sprints(701).unwrap();
    assert_eq!(
        sprints,
        vec![
            Sprint { name: "22 Las vistas".into(), created: "2026-08-01T00:00:00.000Z".into() },
            Sprint { name: "23 Las questions".into(), created: "2026-08-05T00:00:00.000Z".into() },
        ]
    );

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "GET");
    assert!(captured.path.starts_with("/rest/agile/1.0/board/701/sprint"), "{}", captured.path);
}

/// Shape measured against the real ACC/701 project on 2026-09-10: six issue
/// types, each repeating the same three statuses verbatim.
#[test]
fn project_statuses_dedupes_the_same_status_repeated_across_issue_types() {
    let response = r#"[
        {"name":"Subtask","statuses":[
            {"name":"Tareas por hacer","statusCategory":{"key":"new"}},
            {"name":"Finalizada","statusCategory":{"key":"done"}}
        ]},
        {"name":"Epic","statuses":[
            {"name":"Tareas por hacer","statusCategory":{"key":"new"}},
            {"name":"Finalizada","statusCategory":{"key":"done"}}
        ]}
    ]"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let mut statuses = provider.project_statuses("ACC").unwrap();
    statuses.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
        statuses,
        vec![
            Status { name: "Finalizada".into(), category: "done".into() },
            Status { name: "Tareas por hacer".into(), category: "new".into() },
        ]
    );

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "GET");
    assert_eq!(captured.path, "/rest/api/3/project/ACC/statuses");
}

/// Shape measured against the real Jira instance on 2026-09-10: an
/// instance-wide list, not per-project — `Blocks`/`Relates`/`Duplicate`
/// among others, none named `Depends`.
#[test]
fn link_types_hits_the_instance_wide_endpoint_and_parses_both_phrases() {
    let response = r#"{"issueLinkTypes":[
        {"id":"10000","name":"Blocks","inward":"is blocked by","outward":"blocks"},
        {"id":"10003","name":"Relates","inward":"relates to","outward":"relates to"}
    ]}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let types = provider.link_types().unwrap();

    assert_eq!(
        types,
        vec![
            LinkType { name: "Blocks".into(), outward: "blocks".into(), inward: "is blocked by".into() },
            LinkType { name: "Relates".into(), outward: "relates to".into(), inward: "relates to".into() },
        ]
    );

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "GET");
    assert_eq!(captured.path, "/rest/api/3/issueLinkType");
}

/// Measured against ACC on 2026-09-10, with two throwaway items: a `Blocks`
/// link posted with `outwardIssue: ACC-358, inwardIssue: ACC-359` came back
/// as "ACC-359 blocks ACC-358". The issue that plays the outward phrase goes
/// in `inwardIssue` — the names say which end of the link object each field
/// is, not which phrase its issue says.
#[test]
fn create_link_posts_the_issue_playing_the_outward_phrase_as_inward_issue() {
    let (base, rx) = one_shot(201, "");
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    // ACC-338 blocks ACC-229.
    provider.create_link("Blocks", "ACC-338", "ACC-229").unwrap();

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/rest/api/3/issueLink");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["type"]["name"], "Blocks");
    assert_eq!(body["inwardIssue"]["key"], "ACC-338");
    assert_eq!(body["outwardIssue"]["key"], "ACC-229");
}

#[test]
fn update_title_puts_only_the_summary_field() {
    let (base, rx) = one_shot(204, "");
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    provider.update_title("ACC-355", "Nuevo título").unwrap();

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "PUT");
    assert_eq!(captured.path, "/rest/api/3/issue/ACC-355");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["fields"]["summary"], "Nuevo título");
    assert!(body["fields"].get("description").is_none(), "{body}");
}

#[test]
fn update_body_puts_the_adf_as_the_description_field() {
    let (base, rx) = one_shot(204, "");
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));
    let adf = r#"{"type":"doc","version":1,"content":[]}"#;

    provider.update_body("ACC-355", adf).unwrap();

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "PUT");
    assert_eq!(captured.path, "/rest/api/3/issue/ACC-355");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["fields"]["description"]["type"], "doc");
    assert!(body["fields"].get("summary").is_none(), "{body}");
}

#[test]
fn find_by_title_matches_the_exact_summary_among_the_results() {
    let response = r#"{"issues":[
        {"key":"ACC-1","fields":{"summary":"Vistas de trabajo, borrador"}},
        {"key":"ACC-2","fields":{"summary":"Vistas de trabajo"}}
    ]}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let key = provider.find_by_title("ACC", "Tarea", None, "Vistas de trabajo").unwrap();
    assert_eq!(key.as_deref(), Some("ACC-2"), "full-text can return more than the exact title");

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "GET");
    // `/rest/api/3/search` answers 410 Gone — measured against the real
    // instance on 2026-09-10; `/search/jql` is what replaced it.
    assert!(captured.path.starts_with("/rest/api/3/search/jql?jql="), "{}", captured.path);
    assert!(captured.path.contains("project"), "{}", captured.path);
}

#[test]
fn find_by_title_is_none_when_nothing_matches_exactly() {
    let response = r#"{"issues":[{"key":"ACC-1","fields":{"summary":"algo distinto"}}]}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let key = provider.find_by_title("ACC", "Tarea", None, "Vistas de trabajo").unwrap();
    assert_eq!(key, None);
    rx.recv().unwrap();
}

#[test]
fn find_by_title_refuses_a_title_with_nothing_searchable() {
    let provider = JiraRest::new("http://127.0.0.1:1", Credentials::new("a@b.com", "tok"));
    let err = provider.find_by_title("ACC", "Tarea", None, "---").unwrap_err();
    assert!(format!("{err:#}").contains("---"), "{err:#}");
}

#[test]
fn create_item_posts_project_type_and_summary() {
    let (base, rx) = one_shot(201, r#"{"key":"ACC-403"}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let key = provider.create_item("ACC", "Tarea", None, "Vistas de trabajo", None, None).unwrap();
    assert_eq!(key, "ACC-403");

    let captured = rx.recv().unwrap();
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/rest/api/3/issue");
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["fields"]["project"]["key"], "ACC");
    assert_eq!(body["fields"]["issuetype"]["name"], "Tarea");
    assert_eq!(body["fields"]["summary"], "Vistas de trabajo");
    assert!(body["fields"].get("parent").is_none());
    assert!(body["fields"].get("description").is_none());
}

#[test]
fn create_item_includes_parent_and_description_when_given() {
    let (base, rx) = one_shot(201, r#"{"key":"ACC-404"}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));
    let adf = r#"{"type":"doc","version":1,"content":[]}"#;

    provider.create_item("ACC", "Tarea", None, "x", Some("ACC-100"), Some(adf)).unwrap();

    let captured = rx.recv().unwrap();
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["fields"]["parent"]["key"], "ACC-100");
    assert_eq!(body["fields"]["description"]["type"], "doc");
}

#[test]
fn item_with_no_parent_or_description_leaves_both_absent() {
    let response = r#"{"fields":{"summary":"x","status":{"name":"Abierta"},"issuetype":{"name":"Tarea"}}}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let item = provider.item("ACC-1").unwrap();
    assert_eq!(item.parent, None);
    assert_eq!(item.body_adf, None);
    rx.recv().unwrap();
}

#[test]
fn find_by_title_narrows_by_the_label_when_there_is_one() {
    let (base, rx) = one_shot(200, r#"{"issues":[]}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    provider.find_by_title("ACC", "Tarea", Some("question"), "¿se hereda?").unwrap();

    let captured = rx.recv().unwrap();
    assert!(captured.path.contains("labels"), "{}", captured.path);
}

#[test]
fn create_item_sends_the_label_when_there_is_one() {
    let (base, rx) = one_shot(201, r#"{"key":"ACC-403"}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    provider.create_item("ACC", "Tarea", Some("question"), "¿se hereda?", None, None).unwrap();

    let body: serde_json::Value = serde_json::from_str(&rx.recv().unwrap().body).unwrap();
    assert_eq!(body["fields"]["labels"], serde_json::json!(["question"]));
}

#[test]
fn item_reads_the_labels() {
    let response = r#"{"fields":{"summary":"x","status":{"name":"Abierta"},"issuetype":{"name":"Tarea"},"labels":["question"]}}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let item = provider.item("ACC-403").unwrap();

    assert_eq!(item.labels, vec!["question".to_string()]);
    assert!(rx.recv().unwrap().path.contains("labels"));
}

/// Shape measured on SGE-7699 on 2026-09-10: `id` a string, `parentId` a
/// number on a reply and absent — or `null` — on a root. Only this endpoint
/// carries `parentId`; the issue's own `comment` field doesn't.
#[test]
fn comments_reads_each_one_with_the_comment_it_replies_to() {
    let response = r#"{"startAt":0,"maxResults":100,"total":2,"comments":[
        {"id":"42180","author":{"accountId":"a-1","displayName":"Ana"},"created":"2026-09-02T13:06:47.823-0300",
         "body":{"type":"doc","version":1,"content":[]}},
        {"id":"42224","parentId":42180,"author":{"accountId":"b-2","displayName":"Beto"},"created":"2026-09-02T15:47:55.222-0300",
         "body":{"type":"doc","version":1,"content":[]}}
    ]}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let comments = provider.comments("SGE-7699").unwrap();

    let doc = r#"{"content":[],"type":"doc","version":1}"#.to_string();
    assert_eq!(
        comments,
        vec![
            Comment { id: "42180".into(), author: "Ana".into(), author_id: "a-1".into(), created: "2026-09-02T13:06:47.823-0300".into(), parent: None, body_adf: doc.clone() },
            Comment { id: "42224".into(), author: "Beto".into(), author_id: "b-2".into(), created: "2026-09-02T15:47:55.222-0300".into(), parent: Some("42180".into()), body_adf: doc },
        ]
    );
    let captured = rx.recv().unwrap();
    assert!(captured.path.starts_with("/rest/api/3/issue/SGE-7699/comment"), "{}", captured.path);
}

#[test]
fn item_reads_the_attachments() {
    let response = r#"{"fields":{"summary":"x","status":{"name":"Abierta"},"issuetype":{"name":"Mejora"},
        "attachment":[{"id":"44892","filename":"captura.png","mimeType":"image/png","size":34828}]}}"#;
    let (base, rx) = one_shot(200, response);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let item = provider.item("SGE-7699").unwrap();

    assert_eq!(item.attachments, vec![Attachment { id: "44892".into(), filename: "captura.png".into() }]);
    assert!(rx.recv().unwrap().path.contains("attachment"));
}

/// Measured on 2026-09-10: without `redirect=false` the provider answers 303
/// toward another host; with it, the file itself, byte for byte.
#[test]
fn attachment_content_asks_for_the_bytes_without_a_redirect() {
    let (base, rx) = one_shot(200, "the bytes");
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let bytes = provider.attachment_content("44892").unwrap();

    assert_eq!(bytes, b"the bytes");
    assert_eq!(rx.recv().unwrap().path, "/rest/api/3/attachment/content/44892?redirect=false");
}

/// Measured on a throwaway item, ACC-360, on 2026-09-10: the POST takes
/// `parentId`, as the number the comments endpoint hands back.
#[test]
fn add_comment_posts_the_body_and_the_comment_it_replies_to() {
    let (base, rx) = one_shot(201, r#"{"id":"42581","parentId":42580}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));
    let adf = r#"{"version":1,"type":"doc","content":[]}"#;

    let id = provider.add_comment("ACC-360", adf, Some("42580")).unwrap();

    assert_eq!(id, "42581");
    let captured = rx.recv().unwrap();
    assert_eq!((captured.method.as_str(), captured.path.as_str()), ("POST", "/rest/api/3/issue/ACC-360/comment"));
    let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["parentId"], serde_json::json!(42580));
    assert_eq!(body["body"]["type"], "doc");
}

#[test]
fn add_comment_with_no_parent_sends_none() {
    let (base, rx) = one_shot(201, r#"{"id":"42580"}"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    provider.add_comment("ACC-360", r#"{"version":1,"type":"doc","content":[]}"#, None).unwrap();

    let body: serde_json::Value = serde_json::from_str(&rx.recv().unwrap().body).unwrap();
    assert!(body.get("parentId").is_none(), "{body}");
}

/// Measured on ACC-360: a multipart POST, with `X-Atlassian-Token: no-check`
/// — without it the provider refuses the upload as a possible forgery.
#[test]
fn add_attachment_uploads_the_file_as_multipart() {
    let (base, rx) = one_shot(200, r#"[{"id":"45078","filename":"prueba.txt","size":5}]"#);
    let provider = JiraRest::new(base, Credentials::new("a@b.com", "tok"));

    let attachment = provider.add_attachment("ACC-360", "prueba.txt", b"hola\n").unwrap();

    assert_eq!(attachment, Attachment { id: "45078".into(), filename: "prueba.txt".into() });
    let captured = rx.recv().unwrap();
    assert_eq!((captured.method.as_str(), captured.path.as_str()), ("POST", "/rest/api/3/issue/ACC-360/attachments"));
    let raw = captured.raw.to_lowercase();
    assert!(raw.contains("x-atlassian-token: no-check"), "{}", captured.raw);
    assert!(raw.contains("content-type: multipart/form-data; boundary="), "{}", captured.raw);
    assert!(captured.raw.contains("filename=\"prueba.txt\""), "{}", captured.raw);
    assert!(captured.raw.contains("hola\n"), "{}", captured.raw);
}
