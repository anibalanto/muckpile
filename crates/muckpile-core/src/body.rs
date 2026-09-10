//! Converting an item's body to and from the provider's rich-text format
//! (ADF), and deciding whether a body can be edited locally at all.

use amdc::{convert, format::Options, Format, WarningKind};
use anyhow::{Context, Result};

/// Frontmatter and body. The frontmatter never passes through the converter:
/// it isn't markdown, and it doesn't live in the provider's description.
pub fn split_frontmatter(text: &str) -> (&str, &str) {
    if !text.starts_with("---\n") {
        return ("", text);
    }
    match text[4..].find("\n---\n") {
        Some(i) => text.split_at(4 + i + 5),
        None => ("", text),
    }
}

/// Drops the marks the provider's schema refuses to combine.
///
/// In ADF `code` is exclusive: it cannot coexist with `strong` or `em`. GFM
/// lets them nest — `**`x`**`, bold over an identifier — and the provider
/// rejects the whole document for one such node, not just that node.
///
/// `code` wins and the emphasis marks are dropped: `code` carries
/// information — it says this is an identifier — while emphasis can be lost
/// without changing what the sentence means.
pub fn prune_marks(node: &mut serde_json::Value) {
    const EXCLUDED_BY_CODE: [&str; 2] = ["strong", "em"];

    if let Some(marks) = node.get_mut("marks").and_then(|m| m.as_array_mut()) {
        let has_code = marks
            .iter()
            .any(|m| m.get("type").and_then(|t| t.as_str()) == Some("code"));
        if has_code {
            marks.retain(|m| {
                let t = m.get("type").and_then(|t| t.as_str()).unwrap_or_default();
                !EXCLUDED_BY_CODE.contains(&t)
            });
        }
    }
    if let Some(content) = node.get_mut("content").and_then(|c| c.as_array_mut()) {
        for child in content.iter_mut() {
            prune_marks(child);
        }
    }
}

/// The body, as ADF, ready to send to the provider.
pub fn body_to_adf(body: &str) -> Result<String> {
    let out = convert(body, Format::Gfm, Format::Adf, &Options::default())
        .context("converting the body to ADF")?;
    let mut doc = as_json(&out.text)?;
    prune_marks(&mut doc);
    Ok(serde_json::to_string(&doc)?)
}

/// The ADF back to markdown.
pub fn adf_to_body(adf: &str) -> Result<String> {
    let out = convert(adf, Format::Adf, Format::Gfm, &Options::default())
        .context("converting the ADF to markdown")?;
    Ok(out.text)
}

/// Why a body read from the provider can't be edited locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loss {
    /// The converter could only approximate a construct, and said so — named
    /// the way the converter names it: `"numbered table column"`.
    Lossy(String),
    /// Back through markdown, the document isn't the one the converter reads
    /// the provider's ADF as, and nothing warned about it.
    Differs,
}

/// A body read from the provider: its markdown, and every reason editing
/// that markdown locally would lose something — none when it's canonical.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filtered {
    pub markdown: String,
    pub losses: Vec<Loss>,
}

impl Filtered {
    pub fn is_canonical(&self) -> bool {
        self.losses.is_empty()
    }
}

/// From the provider's ADF to markdown, deciding on the way whether the body
/// is canonical: whether its markdown converts back to the same document.
///
/// "The same" is the converter's own reading of the ADF — ADF to ADF — not
/// the ADF as the provider stored it, and compared as JSON. What the
/// converter normalizes on reading (an empty `attrs`, a paragraph's
/// `localId`, a space at the edge of an emphasis run) is an equivalence, so
/// it never counts as a difference; that knowledge lives in the converter,
/// and there are no rules of its own here. A `Lossy` warning on either leg
/// is a loss even when the JSON happens to agree.
pub struct JiraAdfMarkdownFilter;

impl JiraAdfMarkdownFilter {
    pub fn filter(adf: &str) -> Result<Filtered> {
        let options = Options::default();
        let markdown = convert(adf, Format::Adf, Format::Gfm, &options).context("converting the ADF to markdown")?;
        let back = convert(&markdown.text, Format::Gfm, Format::Adf, &options).context("converting the markdown back to ADF")?;

        let mut losses: Vec<Loss> = markdown
            .warnings
            .iter()
            .chain(&back.warnings)
            .filter(|w| w.kind == WarningKind::Lossy)
            .map(|w| match &w.detail {
                Some(detail) => Loss::Lossy(format!("{} ({detail})", w.construct)),
                None => Loss::Lossy(w.construct.clone()),
            })
            .collect();
        // A warning already names what changed; `Differs` is for when
        // something did and nothing said so.
        if losses.is_empty() && as_json(&back.text)? != converter_canonical(adf)? {
            losses.push(Loss::Differs);
        }
        Ok(Filtered { markdown: markdown.text, losses })
    }
}

/// The provider's ADF as the converter reads it — ADF to ADF.
fn converter_canonical(adf: &str) -> Result<serde_json::Value> {
    let out = convert(adf, Format::Adf, Format::Adf, &Options::default()).context("reading the ADF")?;
    as_json(&out.text)
}

fn as_json(adf: &str) -> Result<serde_json::Value> {
    serde_json::from_str(adf).with_context(|| format!("the ADF the converter produced isn't JSON: {adf}"))
}

/// What sending `draft` would do to the provider's body: a line diff of the
/// provider's ADF against the draft as it would be sent, both as indented
/// JSON. The provider's side is in the converter's canonical form, so what
/// the converter normalizes on reading — an empty `attrs` on every table
/// cell — doesn't bury the difference that matters.
pub fn adf_diff(real_adf: &str, draft: &str) -> Result<String> {
    let real = serde_json::to_string_pretty(&converter_canonical(real_adf)?)?;
    let sent = serde_json::to_string_pretty(&as_json(&body_to_adf(draft)?)?)?;
    Ok(line_diff(&real, &sent))
}

/// The canonical form of a file's body: markdown to ADF and back, with the
/// same converter, frontmatter left untouched.
///
/// Comparing two bodies in canonical form is what tells apart *the same
/// thing written another way* from *someone wrote something else* — a diff
/// in canonical form is a diff that matters. It exists so a client can
/// compute it without knowing anything about the provider: no live document
/// to compare against, no network call.
pub fn canonical(text: &str) -> Result<String> {
    let (frontmatter, body) = split_frontmatter(text);
    Ok(format!("{frontmatter}{}", adf_to_body(&body_to_adf(body)?)?))
}

/// A line-by-line diff of `before` against `after`, `git diff`-flavored but
/// with no external tool behind it: ` ` for a line kept, `-` for one only in
/// `before`, `+` for one only in `after`. Meant for a person to read before
/// applying an edit by hand somewhere this system won't write to directly —
/// so it only ever needs to explain the difference, not to be reapplied by
/// a machine.
pub fn line_diff(before: &str, after: &str) -> String {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();

    // Longest common subsequence, by length, over the two line arrays —
    // classic DP table, cheap here since an item's body is never large.
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(format!("  {}", a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(format!("- {}", a[i]));
            i += 1;
        } else {
            out.push(format!("+ {}", b[j]));
            j += 1;
        }
    }
    for line in &a[i..] {
        out.push(format!("- {line}"));
    }
    for line in &b[j..] {
        out.push(format!("+ {line}"));
    }
    out.join("\n")
}
