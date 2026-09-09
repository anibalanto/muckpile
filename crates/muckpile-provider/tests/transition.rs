//! `transition` replaces `start`/`done`/`close`/`drop`: there's no vocabulary
//! of its own to hand them, so one command lists the workflow's own
//! transitions and fires the one that leads where asked.

use muckpile_provider::fake::FakeProvider;
use muckpile_provider::provider::Transition;
use muckpile_provider::transition::{transition, Outcome};

fn t(id: &str, name: &str, to: &str) -> Transition {
    Transition { id: id.to_string(), name: name.to_string(), to: to.to_string() }
}

/// The case the whole command exists for: the transition's name isn't the
/// status it leads to, and choosing has to go by `to`, not by guessing which
/// of the two strings to type.
#[test]
fn the_transition_whose_name_differs_from_its_destination_is_found_and_applied() {
    let p = FakeProvider::new();
    p.seed("ACC-355", "Tareas por hacer", vec![t("31", "Listo", "Finalizada")]);

    let outcome = transition(&p, "ACC-355", "Finalizada").unwrap();
    match outcome {
        Outcome::Applied { transition_name } => assert_eq!(transition_name, "Listo"),
        Outcome::NoSuchTransition { .. } => panic!("expected the transition to apply"),
    }
    assert_eq!(p.status_of("ACC-355").as_deref(), Some("Finalizada"));
}

#[test]
fn a_status_the_workflow_does_not_reach_from_here_lists_what_it_does_reach() {
    let p = FakeProvider::new();
    p.seed("ACC-360", "En curso", vec![t("1", "Bloquear", "Bloqueada"), t("2", "Cerrar", "Cerrada")]);

    let outcome = transition(&p, "ACC-360", "Finalizada").unwrap();
    match outcome {
        Outcome::NoSuchTransition { available } => {
            assert_eq!(available, vec!["Bloqueada".to_string(), "Cerrada".to_string()]);
        }
        Outcome::Applied { .. } => panic!("there was no path to Finalizada"),
    }
    assert_eq!(p.status_of("ACC-360").as_deref(), Some("En curso"), "a rejected transition must not move the item");
}

#[test]
fn applying_asks_by_id_never_by_name() {
    // Two transitions that lead to different statuses but share no name
    // collision with the target: only the `to` match should ever fire.
    let p = FakeProvider::new();
    p.seed("ACC-1", "Abierta", vec![t("9", "Finalizada", "Bloqueada"), t("10", "Terminar", "Finalizada")]);

    let outcome = transition(&p, "ACC-1", "Finalizada").unwrap();
    match outcome {
        Outcome::Applied { transition_name } => assert_eq!(transition_name, "Terminar"),
        Outcome::NoSuchTransition { .. } => panic!("expected the transition to apply"),
    }
}
