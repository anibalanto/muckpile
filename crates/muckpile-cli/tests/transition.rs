//! `transition` replaces `start`/`done`/`close`/`drop` — the
//! deciding-and-firing logic already lives in `muckpile-provider::transition`;
//! this slice is only the id validation the other CLI commands already do.

use muckpile_cli::transition;
use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::Transition;
use muckpile_provider::transition::Outcome;

fn t(id: &str, name: &str, to: &str) -> Transition {
    Transition { id: id.to_string(), name: name.to_string(), to: to.to_string() }
}

#[test]
fn applies_the_transition_leading_to_the_requested_status() {
    let provider = FakeProvider::new();
    provider.seed("ACC-355", "Tareas por hacer", vec![t("31", "Listo", "Finalizada")]);

    let outcome = transition("ACC-355", "Finalizada", &provider).unwrap();

    match outcome {
        Outcome::Applied { transition_name } => assert_eq!(transition_name, "Listo"),
        Outcome::NoSuchTransition { .. } => panic!("expected the transition to apply"),
    }
    assert_eq!(provider.status_of("ACC-355").as_deref(), Some("Finalizada"));
}

#[test]
fn reports_what_the_workflow_does_reach_when_the_target_is_not_one_of_them() {
    let provider = FakeProvider::new();
    provider.seed("ACC-360", "En curso", vec![t("1", "Cerrar", "Cerrada")]);

    let outcome = transition("ACC-360", "Finalizada", &provider).unwrap();

    match outcome {
        Outcome::NoSuchTransition { available } => assert_eq!(available, vec!["Cerrada".to_string()]),
        Outcome::Applied { .. } => panic!("there was no path to Finalizada"),
    }
}

#[test]
fn refuses_an_id_with_characters_a_path_cannot_carry() {
    let provider = FakeProvider::new();
    assert!(transition("../escape", "Finalizada", &provider).is_err());
}
