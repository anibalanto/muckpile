//! The alphabet of an id, and the marker of what's local.

#[test]
fn a_bare_integer_is_not_read_as_unassigned() {
    assert!(!muckpile_core::is_unassigned("10"), "it's provider key 10");
    assert!(muckpile_core::is_unassigned("@10"), "this one is marked");
}

#[test]
fn the_marker_is_the_only_thing_that_distinguishes_an_unassigned_id() {
    assert!(muckpile_core::is_unassigned("@fix-the-hook"));
    assert!(!muckpile_core::is_unassigned("ACC-347"));
    // And it knows no provider: key shape never enters the decision, for
    // either a hit or a miss.
    assert!(!muckpile_core::is_unassigned("PROJ_42/beta"));
}

#[test]
fn the_alphabet_takes_the_marker_once_and_up_front() {
    for id in ["@a", "@fix-the-hook", "ACC-347", "5p", "with_underscore"] {
        assert!(muckpile_core::is_valid_id(id), "{id} should be valid");
    }
    for id in ["", "@", "@@a", "a@b", "a.b", "to-work/20", "with space", "accent-á"] {
        assert!(!muckpile_core::is_valid_id(id), "{id} should not be valid");
    }
}

/// `.` separates the type and `/` says the item doesn't live flat in the
/// view: both break the filename, not anyone's taste.
#[test]
fn the_dot_and_the_slash_are_out_for_what_they_break() {
    assert!(!muckpile_core::is_valid_id("@a.question"), "would it be `@a` the question or `@a.question` the item");
    assert!(!muckpile_core::is_valid_id("to-work/20"), "items live flat in a view");
}
