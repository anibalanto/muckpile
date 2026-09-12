//! `help`: the commands, grouped, one per line with what each does — and a
//! mistyped command said in one line, not answered with the whole list.

use muckpile_cli::{help, usage_of};
use std::process::{Command, Output};

/// Every command by how its usage line starts. Two that share the first
/// word — `sprint fetch` and `sprint create` — are told apart by the second,
/// which is how whoever reads the help tells them apart too.
const EVERY_COMMAND: [&str; 20] = [
    "init", "to-work", "code-work add", "sprint fetch", "sprint create", "sprint add", "states discover", "show", "list", "status", "new", "pull",
    "push", "title", "transition", "parent", "link", "unlink", "comment", "attach",
];

#[test]
fn help_lists_every_command_on_its_own_line_with_what_it_does() {
    let text = help();

    for command in EVERY_COMMAND {
        let lines: Vec<&str> = text.lines().filter(|l| l.starts_with(&format!("  {command} ")) || *l == format!("  {command}")).collect();
        assert_eq!(lines.len(), 1, "{command}:\n{text}");
    }
    assert!(!text.contains("help.usage") && !text.contains("help.what") && !text.contains("help.group"), "a message key left as itself:\n{text}");
    assert!(text.lines().all(|l| l.chars().count() <= 100), "a line too long to read:\n{text}");
}

#[test]
fn the_usage_of_one_command_is_one_line() {
    assert_eq!(usage_of("push").as_deref(), Some("uso: muckpile push <vista>"));
    assert_eq!(usage_of("sprint").as_deref(), Some("uso: muckpile sprint fetch"));
    assert_eq!(usage_of("nada"), None);
}

fn muckpile(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_muckpile")).args(args).current_dir(dir.path()).env("MUCKPILE_LANG", "es-AR").output().unwrap()
}

#[test]
fn asking_for_help_is_not_an_error() {
    for args in [&["--help"][..], &["-h"], &["help"], &[]] {
        let out = muckpile(args);
        assert!(out.status.success(), "{args:?}");
        assert_eq!(String::from_utf8_lossy(&out.stdout), help(), "{args:?}");
        assert!(out.stderr.is_empty(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn a_command_that_does_not_exist_is_said_in_one_line() {
    let out = muckpile(&["hacer"]);

    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("hacer") && err.contains("--help"), "{err}");
    assert!(!err.contains("attach"), "not the whole list: {err}");
}

/// Two commands share their first word, and the usage shown is the one
/// asked for: `sprint create` with no name doesn't explain `sprint fetch`.
#[test]
fn a_two_word_command_shows_its_own_usage() {
    let out = muckpile(&["sprint", "create"]);

    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("uso: muckpile sprint create <nombre>"), "{err}");
}

#[test]
fn a_command_with_the_wrong_arguments_shows_only_its_usage() {
    let out = muckpile(&["push"]);

    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("uso: muckpile push <vista>"), "{err}");
    assert!(!err.contains("attach"), "not the whole list: {err}");
}
