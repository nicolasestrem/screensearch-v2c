//! Versioned D6/D7 recognition taxonomy. Entries are evaluated in file order.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use traits::{SessionHost, SessionKind};

const SEED_TOML: &str = include_str!("../taxonomy.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recognized {
    pub id: String,
    pub kind: SessionKind,
    pub host: Option<SessionHost>,
}

#[derive(Debug, Clone, Deserialize)]
struct Entry {
    id: String,
    kind: SessionKind,
    #[serde(default)]
    host: Option<SessionHost>,
    #[serde(default)]
    app_hints: Vec<String>,
    #[serde(default)]
    title_patterns: Vec<String>,
    #[serde(default)]
    title_prefix_ranges: Vec<String>,
    /// Explicit negative title evidence. Added for gap #117's ChatGPT→Codex rename:
    /// app stem `chatgpt` means Codex except the distinct `ChatGPT Classic` app title.
    #[serde(default)]
    title_exclude_patterns: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    domains: Vec<String>,
    #[serde(skip)]
    prefix_ranges: Vec<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub(crate) struct Taxonomy {
    pub version: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct RawTaxonomy {
    version: u32,
    #[serde(default, rename = "entry")]
    entries: Vec<Entry>,
}

fn normalize_stem(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .map_or(lower.clone(), str::to_string)
}

fn parse_hex_scalar(value: &str) -> Result<u32> {
    let value = value.trim();
    let scalar =
        u32::from_str_radix(value, 16).with_context(|| format!("invalid hex {value:?}"))?;
    char::from_u32(scalar)
        .with_context(|| format!("hex {value:?} is not a Unicode scalar value"))?;
    Ok(scalar)
}

fn parse_prefix_ranges(specs: &[String]) -> Result<Vec<(u32, u32)>> {
    specs
        .iter()
        .map(|spec| {
            let spec = spec.trim();
            let (lo, hi) = match spec.split_once('-') {
                Some((lo, hi)) => (parse_hex_scalar(lo)?, parse_hex_scalar(hi)?),
                None => {
                    let value = parse_hex_scalar(spec)?;
                    (value, value)
                }
            };
            if lo > hi {
                bail!("prefix range {spec:?} has lo > hi");
            }
            Ok((lo, hi))
        })
        .collect()
}

impl Taxonomy {
    fn parse(doc: &str) -> Result<Self> {
        let mut raw: RawTaxonomy = toml::from_str(doc).context("parsing taxonomy.toml")?;
        if raw.version != 3 {
            bail!("unsupported taxonomy version {}; expected 3", raw.version);
        }
        for entry in &mut raw.entries {
            if entry.id.trim().is_empty() {
                bail!("taxonomy entry has an empty id");
            }
            if entry.app_hints.is_empty()
                && entry.title_patterns.is_empty()
                && entry.title_prefix_ranges.is_empty()
            {
                bail!(
                    "taxonomy entry {:?} has no app_hints, title_patterns, or title_prefix_ranges",
                    entry.id
                );
            }
            for hint in &mut entry.app_hints {
                *hint = normalize_stem(hint);
            }
            for pattern in &mut entry.title_patterns {
                *pattern = pattern.to_lowercase();
            }
            for pattern in &mut entry.title_exclude_patterns {
                *pattern = pattern.to_lowercase();
            }
            entry.prefix_ranges = parse_prefix_ranges(&entry.title_prefix_ranges)
                .with_context(|| format!("taxonomy entry {:?} title_prefix_ranges", entry.id))?;
        }
        Ok(Self {
            version: raw.version,
            entries: raw.entries,
        })
    }

    pub(crate) fn seed() -> Result<Self> {
        Self::parse(SEED_TOML).context("bundled sessions taxonomy")
    }

    pub(crate) fn recognize(
        &self,
        app_hint: Option<&str>,
        title: Option<&str>,
    ) -> Option<Recognized> {
        let app = app_hint.map(normalize_stem);
        let title = title.map(|value| value.to_lowercase());
        if app.is_none() && title.is_none() {
            return None;
        }
        let title_prefix = title
            .as_deref()
            .and_then(|value| value.chars().find(|c| !c.is_whitespace()))
            .map(|c| c as u32);

        for entry in &self.entries {
            let app_ok = entry.app_hints.is_empty()
                || app
                    .as_ref()
                    .is_some_and(|app| entry.app_hints.iter().any(|hint| app == hint));
            let title_ok = (entry.title_patterns.is_empty() && entry.prefix_ranges.is_empty())
                || title.as_ref().is_some_and(|title| {
                    entry
                        .title_patterns
                        .iter()
                        .any(|pattern| title.contains(pattern))
                })
                || title_prefix.is_some_and(|scalar| {
                    entry
                        .prefix_ranges
                        .iter()
                        .any(|&(lo, hi)| scalar >= lo && scalar <= hi)
                });
            let excluded = title.as_ref().is_some_and(|title| {
                entry
                    .title_exclude_patterns
                    .iter()
                    .any(|pattern| title.contains(pattern))
            });
            if app_ok && title_ok && !excluded {
                return Some(Recognized {
                    id: entry.id.clone(),
                    kind: entry.kind,
                    host: entry.host,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_prefix_range_is_rejected() {
        let invalid = r#"
version = 3
[[entry]]
id = "x"
kind = "ai"
app_hints = ["wt"]
title_prefix_ranges = ["2800-2700"]
"#;
        assert!(Taxonomy::parse(invalid).is_err());
    }

    #[test]
    fn seed_has_nine_entries() {
        let taxonomy = Taxonomy::seed().unwrap();
        assert_eq!(taxonomy.version, 3);
        assert_eq!(taxonomy.entries.len(), 9);
    }

    #[test]
    fn parsing_normalizes_match_inputs_once() {
        let doc = r#"
version = 3
[[entry]]
id = "x"
kind = "ai"
app_hints = ["  Example.EXE  "]
title_patterns = ["Mixed Case"]
title_exclude_patterns = ["Do Not Match"]
"#;
        let taxonomy = Taxonomy::parse(doc).unwrap();
        let entry = &taxonomy.entries[0];

        assert_eq!(entry.app_hints, ["example"]);
        assert_eq!(entry.title_patterns, ["mixed case"]);
        assert_eq!(entry.title_exclude_patterns, ["do not match"]);
    }
}
