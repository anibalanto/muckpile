//! What the user reads, in their language. The code names each message by
//! an English key; its text lives in one file per language —
//! `i18n/en.toml` and `i18n/es-AR.toml`, embedded in the binary — with the
//! message's data between braces, `{id}`. Use `msg!` to build one.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// The languages there is a message file for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    EsAr,
}

/// The language `t` speaks, as a `Lang` discriminant. es-AR until someone
/// chooses: the library's own tests read its messages in es-AR whatever
/// the machine's locale says; only the binary asks the environment.
static CURRENT: AtomicU8 = AtomicU8::new(Lang::EsAr as u8);

/// Chooses the language `t` and `msg!` speak from here on, for the whole
/// process.
pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::En as u8 {
        Lang::En
    } else {
        Lang::EsAr
    }
}

/// The language the environment asks for: `lang_for` over `MUCKPILE_LANG`,
/// `LC_ALL`, `LC_MESSAGES` and `LANG`.
pub fn lang_from_env() -> Lang {
    let var = |name| std::env::var(name).ok();
    lang_for(var("MUCKPILE_LANG").as_deref(), var("LC_ALL").as_deref(), var("LC_MESSAGES").as_deref(), var("LANG").as_deref())
}

/// The first of these that's set and not empty decides — `MUCKPILE_LANG`,
/// since a machine's locale isn't always its user's language, and then the
/// locale as the system reads it: one starting with `es` is es-AR, anything
/// else, or nothing at all, English.
pub fn lang_for(muckpile_lang: Option<&str>, lc_all: Option<&str>, lc_messages: Option<&str>, lang: Option<&str>) -> Lang {
    match [muckpile_lang, lc_all, lc_messages, lang].into_iter().flatten().find(|v| !v.is_empty()) {
        Some(v) if v.starts_with("es") => Lang::EsAr,
        _ => Lang::En,
    }
}

/// One language's messages, key to text. A file may write a key dotted —
/// `push.sent = "…"` — or under a table, `[push]` then `sent = "…"`: TOML
/// reads both the same, and so does this.
#[derive(Debug, Default)]
pub struct Catalog {
    messages: BTreeMap<String, String>,
}

impl Catalog {
    /// Reads a message file. Everything in it has to be text.
    pub fn parse(text: &str) -> Result<Catalog> {
        let table: toml::Table = toml::from_str(text).context("parsing a message file")?;
        let mut messages = BTreeMap::new();
        flatten("", &table, &mut messages)?;
        Ok(Catalog { messages })
    }

    /// The text of `key`, placeholders unfilled.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }

    /// Every key, in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.messages.keys().map(String::as_str)
    }
}

fn flatten(prefix: &str, table: &toml::Table, out: &mut BTreeMap<String, String>) -> Result<()> {
    for (key, value) in table {
        let key = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
        match value {
            toml::Value::String(text) => {
                out.insert(key, text.clone());
            }
            toml::Value::Table(inner) => flatten(&key, inner, out)?,
            other => anyhow::bail!("{key}: a message is text, not {}", other.type_str()),
        }
    }
    Ok(())
}

/// The messages the binary carries for `lang`, read once. A file that
/// doesn't parse — which the tests rule out — leaves that language with no
/// messages rather than stopping the program: `t` falls back to English,
/// or to the key.
pub fn catalog(lang: Lang) -> &'static Catalog {
    static EN: OnceLock<Catalog> = OnceLock::new();
    static ES_AR: OnceLock<Catalog> = OnceLock::new();
    match lang {
        Lang::En => EN.get_or_init(|| Catalog::parse(include_str!("../i18n/en.toml")).unwrap_or_default()),
        Lang::EsAr => ES_AR.get_or_init(|| Catalog::parse(include_str!("../i18n/es-AR.toml")).unwrap_or_default()),
    }
}

/// The message `key` from the first of `catalogs` that has it, its
/// placeholders filled from `args` — or, when none has it, the key itself:
/// a missing message never stops the program.
pub fn translate(catalogs: &[&Catalog], key: &str, args: &[(&str, &dyn Display)]) -> String {
    match catalogs.iter().find_map(|c| c.get(key)) {
        Some(text) => fill(text, args),
        None => key.to_string(),
    }
}

/// The message `key` in `lang`, falling back to English when `lang` lacks it.
pub fn t_in(lang: Lang, key: &str, args: &[(&str, &dyn Display)]) -> String {
    match lang {
        Lang::En => translate(&[catalog(Lang::En)], key, args),
        Lang::EsAr => translate(&[catalog(Lang::EsAr), catalog(Lang::En)], key, args),
    }
}

/// The message `key` in the language `set_lang` chose — es-AR if nobody did.
pub fn t(key: &str, args: &[(&str, &dyn Display)]) -> String {
    t_in(current(), key, args)
}

/// Replaces each `{name}` in `text` with the datum of that name, in one
/// pass — what a datum says is never read as a placeholder. A placeholder
/// with no datum is left as written.
fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let datum = after.find('}').and_then(|close| args.iter().find(|(name, _)| *name == &after[..close]).map(|(_, value)| (close, value)));
        match datum {
            Some((close, value)) => {
                out.push_str(&value.to_string());
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A message in the current language: `msg!("id.invalid", id)` or
/// `msg!("pull.brought", path = rel.display())` — each datum named the way
/// `format!` names one, by a variable alone or `name = value`, and filling
/// the `{name}` of the same name.
#[macro_export]
macro_rules! msg {
    (@value $name:ident = $value:expr) => {
        $value
    };
    (@value $name:ident) => {
        $name
    };
    ($key:expr $(, $name:ident $(= $value:expr)?)* $(,)?) => {
        $crate::i18n::t($key, &[$((stringify!($name), &$crate::msg!(@value $name $(= $value)?) as &dyn ::std::fmt::Display)),*])
    };
}
