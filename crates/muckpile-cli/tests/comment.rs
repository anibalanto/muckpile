//! `comment` and `attach`: the thread and the files are written by command,
//! and every comment says who wrote it — a model, or a person.

use muckpile_cli::{attach, comment, confirm_human, random_phrase, Author};
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::Provider;

/// The person approved it, or the project writes on its own.
fn approved() -> anyhow::Result<()> {
    Ok(())
}

/// A person at a terminal, retyping exactly what they were shown.
fn person() -> muckpile_cli::HumanProof {
    confirm_human(|phrase| Ok(phrase.to_string())).unwrap()
}

fn draft(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let path = dir.join("respuesta.md");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn refuses_a_comment_that_does_not_say_who_wrote_it() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);

    let err = comment("ACC-360", &draft(dir.path(), "algo\n"), None, None, &provider).unwrap_err();

    assert!(err.to_string().contains("--ai") && err.to_string().contains("--i-human"), "{err}");
    assert!(provider.comments("ACC-360").unwrap().is_empty());
}

/// What `--ai` writes is what `pull` reads back into the header: the model
/// as the comment's first paragraph, as code.
#[test]
fn an_ai_comment_opens_with_the_model() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);

    let id = comment("ACC-360", &draft(dir.path(), "lo que dijo\n"), None, Some(Author::Ai("claude-opus-5")), &provider).unwrap();

    let sent = provider.comments("ACC-360").unwrap().into_iter().find(|c| c.id == id).unwrap();
    let markdown = muckpile_core::body::adf_to_body(&sent.body_adf).unwrap();
    assert!(markdown.starts_with("ai: `claude-opus-5`\n\nlo que dijo"), "{markdown}");
}

#[test]
fn a_human_comment_adds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);

    let id = comment("ACC-360", &draft(dir.path(), "lo que dije\n"), None, Some(Author::Human(person())), &provider).unwrap();

    let sent = provider.comments("ACC-360").unwrap().into_iter().find(|c| c.id == id).unwrap();
    assert!(!sent.body_adf.contains("ai: "), "{}", sent.body_adf);
    assert!(sent.body_adf.contains("lo que dije"), "{}", sent.body_adf);
}

#[test]
fn a_reply_hangs_from_the_comment_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);
    let root = comment("ACC-360", &draft(dir.path(), "raíz\n"), None, Some(Author::Human(person())), &provider).unwrap();

    let reply = comment("ACC-360", &draft(dir.path(), "respuesta\n"), Some(&root), Some(Author::Human(person())), &provider).unwrap();

    let sent = provider.comments("ACC-360").unwrap().into_iter().find(|c| c.id == reply).unwrap();
    assert_eq!(sent.parent.as_deref(), Some(root.as_str()));
}

#[test]
fn refuses_a_model_that_would_not_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);

    assert!(comment("ACC-360", &draft(dir.path(), "x\n"), None, Some(Author::Ai("")), &provider).is_err());
    assert!(comment("ACC-360", &draft(dir.path(), "x\n"), None, Some(Author::Ai("a`b")), &provider).is_err());
}

#[test]
fn attach_uploads_the_file_under_its_own_name() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);
    let path = dir.path().join("medicion.txt");
    std::fs::write(&path, "34828 bytes").unwrap();

    let attachment = attach("ACC-360", &path, &provider, approved).unwrap();

    assert_eq!(attachment.filename, "medicion.txt");
    assert_eq!(provider.attachment_content(&attachment.id).unwrap(), b"34828 bytes");
}

#[test]
fn refuses_an_id_with_characters_a_path_cannot_carry() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    assert!(comment("../x", &draft(dir.path(), "x\n"), None, Some(Author::Human(person())), &provider).is_err());
    assert!(attach("../x", &draft(dir.path(), "x\n"), &provider, approved).is_err());
}

#[test]
fn a_person_who_retypes_the_phrase_is_confirmed() {
    assert!(confirm_human(|phrase| Ok(format!("{phrase}\n"))).is_ok(), "the newline a terminal leaves is fine");
}

#[test]
fn anything_but_the_phrase_is_refused() {
    let err = confirm_human(|_| Ok("otra-cosa".to_string())).unwrap_err();
    assert!(err.to_string().contains("--i-human"), "{err}");
    assert!(confirm_human(|_| Ok(String::new())).is_err());
}

/// No terminal to ask on — an agent's shell — is a refusal, not a pass.
#[test]
fn no_terminal_to_ask_on_is_a_refusal() {
    assert!(confirm_human(|_| anyhow::bail!("no terminal")).is_err());
}

#[test]
fn the_phrase_is_two_short_words() {
    for _ in 0..50 {
        let phrase = random_phrase();
        let words: Vec<&str> = phrase.split('-').collect();
        assert_eq!(words.len(), 2, "{phrase}");
        assert!(words.iter().all(|w| !w.is_empty() && w.len() <= 7 && w.chars().all(|c| c.is_ascii_lowercase())), "{phrase}");
    }
}

#[test]
fn the_phrase_is_not_always_the_same() {
    let phrases: std::collections::BTreeSet<String> = (0..20).map(|_| random_phrase()).collect();
    assert!(phrases.len() > 1, "{phrases:?}");
}

#[test]
fn a_link_to_an_item_file_in_a_comment_goes_up_as_a_card() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new();
    provider.seed_item("ACC-360", "Tarea", "x", "Abierta", None, None);

    let id = comment("ACC-360", &draft(dir.path(), "Mirá [ACC-356](ACC-356.task.md).\n"), None, Some(Author::Human(person())), &provider).unwrap();

    let sent = provider.comments("ACC-360").unwrap().into_iter().find(|c| c.id == id).unwrap();
    assert!(sent.body_adf.contains(r#""type":"inlineCard""#), "{}", sent.body_adf);
}
