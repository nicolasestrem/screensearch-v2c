//! D7 recognition taxonomy (`specs/03 §7e`).
//!
//! Parses the versioned TOML data file (`crates/harness/taxonomy.toml`) and recognizes a
//! tool/meeting from a frame's `app_hint` + `window_title`. Matching is case-insensitive
//! substring, entries evaluated in file order, first match wins:
//! - `app_ok`   = `app_hints` empty OR `app_hint` contains one hint
//! - `title_ok` = `title_patterns` empty OR `window_title` contains one pattern
//! - `match`    = `app_ok AND title_ok`
//!
//! An entry whose `app_hints` and `title_patterns` are both empty would match everything and
//! is rejected at parse time. `domains` are carried for the dormant browser-URL refinement
//! (gap #109) and are not consulted today.

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::model::{Host, Kind};

/// The bundled seed taxonomy, compiled into the harness for tests and the default matcher.
const SEED_TOML: &str = include_str!("../taxonomy.toml");

/// A recognition outcome for one frame's context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recognized {
    /// Taxonomy id (`claude-code`, `zoom`, ...). Folded into the segmenter's context key and
    /// used for tool-recognition scoring; for `kind = Ai` it is the session's `tool`.
    pub id: String,
    pub kind: Kind,
    pub host: Option<Host>,
}

#[derive(Debug, Clone, Deserialize)]
struct Entry {
    id: String,
    kind: Kind,
    #[serde(default)]
    host: Option<Host>,
    #[serde(default)]
    app_hints: Vec<String>,
    #[serde(default)]
    title_patterns: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)] // dormant refinement (browser_url is NULL in production, gap #109)
    domains: Vec<String>,
}

/// A parsed recognition taxonomy.
#[derive(Debug, Clone)]
pub struct Taxonomy {
    pub version: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct RawTaxonomy {
    version: u32,
    #[serde(default, rename = "entry")]
    entries: Vec<Entry>,
}

impl Taxonomy {
    /// Parse a taxonomy TOML document, validating that no entry has an empty matcher and that
    /// every `Ai` entry carries an id (a meeting id is used only for scoring).
    pub fn parse(doc: &str) -> Result<Self> {
        let raw: RawTaxonomy = toml::from_str(doc).context("parsing taxonomy.toml")?;
        for e in &raw.entries {
            if e.app_hints.is_empty() && e.title_patterns.is_empty() {
                bail!(
                    "taxonomy entry {:?} has neither app_hints nor title_patterns (would match every frame)",
                    e.id
                );
            }
            if e.id.trim().is_empty() {
                bail!("taxonomy entry has an empty id");
            }
        }
        Ok(Self {
            version: raw.version,
            entries: raw.entries,
        })
    }

    /// The bundled seed taxonomy (`crates/harness/taxonomy.toml`).
    pub fn seed() -> Self {
        Self::parse(SEED_TOML).expect("bundled seed taxonomy.toml is valid")
    }

    /// Recognize a tool/meeting from a frame's context, or `None` if nothing matches.
    pub fn recognize(&self, app_hint: Option<&str>, title: Option<&str>) -> Option<Recognized> {
        let app = app_hint.map(str::to_ascii_lowercase);
        let title = title.map(str::to_ascii_lowercase);
        if app.is_none() && title.is_none() {
            return None;
        }
        let contains_any = |hay: &Option<String>, needles: &[String]| -> bool {
            match hay {
                Some(h) => needles.iter().any(|n| h.contains(n.as_str())),
                None => false,
            }
        };
        for e in &self.entries {
            let app_ok = e.app_hints.is_empty() || contains_any(&app, &e.app_hints);
            let title_ok = e.title_patterns.is_empty() || contains_any(&title, &e.title_patterns);
            if app_ok && title_ok {
                return Some(Recognized {
                    id: e.id.clone(),
                    kind: e.kind,
                    host: e.host,
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
    fn seed_parses_and_has_the_d7_set() {
        let t = Taxonomy::seed();
        assert_eq!(t.version, 1);
        // 6 tools + 5 meetings = 11 seed entries.
        assert_eq!(t.entries.len(), 11);
    }

    #[test]
    fn claude_code_needs_terminal_app_and_claude_title() {
        let t = Taxonomy::seed();
        let r = t
            .recognize(
                Some("WindowsTerminal"),
                Some("\u{2733} claude - screensearch-v2c"),
            )
            .expect("claude in a terminal");
        assert_eq!(r.id, "claude-code");
        assert_eq!(r.kind, Kind::Ai);
        assert_eq!(r.host, Some(Host::Terminal));
    }

    #[test]
    fn plain_terminal_without_ai_title_is_unrecognized() {
        let t = Taxonomy::seed();
        // A terminal whose title names neither claude nor codex is not an AI session.
        assert!(t
            .recognize(Some("WindowsTerminal"), Some("pwsh - repo"))
            .is_none());
    }

    #[test]
    fn codex_in_terminal() {
        let t = Taxonomy::seed();
        let r = t
            .recognize(Some("pwsh"), Some("codex - fixing tests"))
            .expect("codex");
        assert_eq!(r.id, "codex");
        assert_eq!(r.kind, Kind::Ai);
    }

    #[test]
    fn vscode_matches_on_executable_alone() {
        let t = Taxonomy::seed();
        let r = t
            .recognize(
                Some("Code"),
                Some("labels.rs - screensearch-v2c - Visual Studio Code"),
            )
            .expect("vscode");
        assert_eq!(r.id, "vscode");
        assert_eq!(r.host, Some(Host::Ide));
    }

    #[test]
    fn browser_ai_needs_browser_app_and_ai_title() {
        let t = Taxonomy::seed();
        let r = t
            .recognize(Some("chrome"), Some("ChatGPT - Google Chrome"))
            .expect("browser ai");
        assert_eq!(r.id, "browser-ai");
        assert_eq!(r.host, Some(Host::Browser));
        // A browser on a non-AI page is not recognized.
        assert!(t
            .recognize(Some("chrome"), Some("GitHub - Google Chrome"))
            .is_none());
    }

    #[test]
    fn claude_desktop_vs_claude_code_precedence() {
        let t = Taxonomy::seed();
        // The desktop app (app stem "Claude", any title) is claude-desktop, not claude-code
        // (which requires a terminal app), even though the title contains "claude".
        let r = t
            .recognize(Some("Claude"), Some("Claude"))
            .expect("desktop");
        assert_eq!(r.id, "claude-desktop");
        assert_eq!(r.host, Some(Host::Desktop));
    }

    #[test]
    fn meeting_recognition() {
        let t = Taxonomy::seed();
        assert_eq!(
            t.recognize(Some("Zoom"), Some("Zoom Meeting"))
                .expect("zoom")
                .kind,
            Kind::Meeting
        );
        assert_eq!(
            t.recognize(Some("ms-teams"), Some("Weekly sync | Microsoft Teams"))
                .expect("teams")
                .id,
            "teams"
        );
        // Google Meet in a browser is a meeting, distinct from browser-ai.
        assert_eq!(
            t.recognize(Some("chrome"), Some("Google Meet"))
                .expect("meet")
                .id,
            "meet"
        );
    }

    #[test]
    fn case_insensitive_and_none_on_empty_context() {
        let t = Taxonomy::seed();
        assert_eq!(
            t.recognize(Some("CURSOR"), None).expect("cursor").id,
            "cursor"
        );
        assert!(t.recognize(None, None).is_none());
    }

    #[test]
    fn rejects_entry_with_empty_matcher() {
        let doc = r#"
version = 1
[[entry]]
id = "catch-all"
kind = "other"
"#;
        assert!(Taxonomy::parse(doc)
            .unwrap_err()
            .to_string()
            .contains("neither app_hints nor title_patterns"));
    }
}
