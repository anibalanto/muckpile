//! `~/.config/muckpile/identity.toml`: the machine-local half of a project's
//! configuration — which account, and where its token lives.
//! Never the token itself: only the name of the variable that holds it.

use muckpile_core::identity::load_identity;

#[test]
fn reads_the_named_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.toml");
    std::fs::write(
        &path,
        r#"
[projects.sge]
jira_email = "aantonelli@lamansys.com.ar"
jira_token_env = "JIRA_API_TOKEN_LAMANSYS"

[projects.acc]
jira_email = "anibal.anto@gmail.com"
jira_token_env = "JIRA_API_TOKEN"
"#,
    )
    .unwrap();

    let identity = load_identity(&path, "acc").unwrap();
    assert_eq!(identity.jira_email, "anibal.anto@gmail.com");
    assert_eq!(identity.jira_token_env, "JIRA_API_TOKEN");
}

#[test]
fn refuses_a_project_the_file_does_not_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.toml");
    std::fs::write(&path, "[projects.sge]\njira_email = \"a@b.com\"\njira_token_env = \"X\"\n").unwrap();

    let err = load_identity(&path, "acc").unwrap_err();
    assert!(err.to_string().contains("acc"), "{err}");
}
