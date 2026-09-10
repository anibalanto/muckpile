//! Building a JQL `summary ~` clause out of a free-form title.

/// The text to search with, not the title itself.
///
/// `summary ~` is full-text, not a literal comparison, and a handful of
/// characters break its parser outright rather than just failing to match —
/// measured against a real instance, not assumed: `[ ] ( ) { } ^ "` and `*`
/// all either stop the query from parsing or silently return nothing for an
/// issue that exists. Those fall, replaced by a space; everything else
/// (accents included — the search doesn't normalize them) survives.
///
/// A run of two or more hyphens breaks the same parser, but a single hyphen
/// is safe and dropping it would break tokenization the other way — a title
/// like `bilinker-002-file-partition` wouldn't be found by a query for
/// `bilinker 002 file partition`. So it's the *run* that falls, not the
/// character: `graph --format json` searches as `graph format json`, and
/// `bilinker-002` stays intact.
///
/// The query doesn't need to be precise — matching the exact title is the
/// caller's job, comparing the full `summary` field once results come back —
/// it only needs to not fail to parse and not miss the issue it's looking
/// for.
pub fn search_text(title: &str) -> String {
    const BREAKS: [char; 9] = ['[', ']', '(', ')', '{', '}', '^', '"', '*'];
    let mut out = String::with_capacity(title.len());
    let mut space = true;
    let mut chars = title.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            while chars.peek() == Some(&'-') {
                chars.next();
            }
            if !space {
                out.push(' ');
                space = true;
            }
            continue;
        }
        if BREAKS.contains(&c) || c == '\\' {
            if !space {
                out.push(' ');
                space = true;
            }
        } else {
            out.push(c);
            space = c.is_whitespace();
        }
    }
    out.trim().to_string()
}
