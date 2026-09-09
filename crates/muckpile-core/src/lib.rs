pub mod body;

/// The item types a view can hold. `sprint` isn't here: it names a folder
/// under `backlog/sprint/`, never a `<id>.<type>.md` file.
pub const TYPES: [&str; 4] = ["task", "user-story", "epic", "question"];
