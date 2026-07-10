//! D7 recognition taxonomy (`specs/03 §7e`).
//!
//! Parses the versioned TOML data file (`crates/harness/taxonomy.toml`) and recognizes a
//! tool/meeting from a frame's `app_hint` + `window_title`. Entries are evaluated in file
//! order, first match wins:
//! - `app_ok`   = `app_hints` empty OR `app_hint` EQUALS one hint (exact stem, `.exe` stripped,
//!   case-insensitive) — NOT substring, so a `codex` process never collides with a `code` hint
//! - `title_ok` = (`title_patterns` AND `title_prefix_ranges` both empty) OR `window_title`
//!   CONTAINS one pattern (substring) OR the first non-whitespace char of `window_title` has a
//!   Unicode scalar value inside one `title_prefix_ranges` entry
//! - `excluded` = `window_title` CONTAINS one `title_exclude_patterns` entry
//! - `match`    = `app_ok AND title_ok AND NOT excluded`
//!
//! `title_prefix_ranges` (v3, gap #111) expresses "the title starts with a glyph in this set",
//! which the substring matcher cannot: Claude Code writes the terminal title as the current task
//! with a spinner/sparkle prefix (`✳` U+2733, braille `U+2800..=U+28FF`), so `claude-code` adds
//! `["2733", "2800-28FF"]`. It is ANDed to the terminal `app_hints`, confining the spinner rule
//! to terminal stems; the `"claude"` substring stays as a costless fallback. Each range spec is
//! a hex Unicode scalar (`"2733"`) or an inclusive `"lo-hi"` range (`"2800-28FF"`), validated at
//! parse time (valid hex, valid scalar, `lo <= hi`).
//!
//! An entry whose `app_hints`, `title_patterns`, and `title_prefix_ranges` are all empty would
//! match everything and is rejected at parse time. `domains` are carried for the dormant
//! browser-URL refinement (gap #109) and are not consulted today.

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::model::{Host, Kind};

/// The bundled seed taxonomy, compiled into the harness for tests and the default matcher.
const SEED_TOML: &str = include_str!("../../sessions/taxonomy.toml");

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
    /// Hex Unicode scalars / `"lo-hi"` ranges matched against the title's first non-whitespace
    /// char (v3, gap #111). Raw as written in TOML; parsed into `prefix_ranges` at parse time.
    #[serde(default)]
    title_prefix_ranges: Vec<String>,
    /// Explicit negative title evidence used by the tuned v3 ChatGPT→Codex rename decision
    /// (gap #117). Kept in the referee parser so its canonical taxonomy interpretation remains
    /// byte-for-byte equivalent to the shipped provider.
    #[serde(default)]
    title_exclude_patterns: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)] // dormant refinement (browser_url is NULL in production, gap #109)
    domains: Vec<String>,
    /// `title_prefix_ranges` parsed to inclusive `(lo, hi)` scalar ranges. Filled by `parse`
    /// (not deserialized); the matcher reads this, never re-parses hex per frame.
    #[serde(skip)]
    prefix_ranges: Vec<(u32, u32)>,
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

/// Lowercase and strip a trailing `.exe` so app_hints compare as bare process stems.
fn normalize_stem(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    match lower.strip_suffix(".exe") {
        Some(stem) => stem.to_string(),
        None => lower,
    }
}

/// Parse `title_prefix_ranges` specs (`"2733"` or `"2800-28FF"`) into inclusive `(lo, hi)`
/// Unicode scalar ranges, validating hex, scalar validity, and `lo <= hi`.
fn parse_prefix_ranges(specs: &[String]) -> Result<Vec<(u32, u32)>> {
    let mut out = Vec::with_capacity(specs.len());
    for spec in specs {
        let spec = spec.trim();
        let (lo, hi) = match spec.split_once('-') {
            Some((a, b)) => (parse_hex_scalar(a)?, parse_hex_scalar(b)?),
            None => {
                let v = parse_hex_scalar(spec)?;
                (v, v)
            }
        };
        if lo > hi {
            bail!("prefix range {spec:?} has lo > hi");
        }
        out.push((lo, hi));
    }
    Ok(out)
}

/// Parse one hex string into a Unicode scalar value (rejecting non-hex and surrogate/out-of-range).
fn parse_hex_scalar(s: &str) -> Result<u32> {
    let s = s.trim();
    let v = u32::from_str_radix(s, 16).with_context(|| format!("invalid hex {s:?}"))?;
    char::from_u32(v).with_context(|| format!("hex {s:?} is not a Unicode scalar value"))?;
    Ok(v)
}

impl Taxonomy {
    /// Parse a taxonomy TOML document, validating that no entry has an empty matcher and that
    /// every `Ai` entry carries an id (a meeting id is used only for scoring).
    pub fn parse(doc: &str) -> Result<Self> {
        let mut raw: RawTaxonomy = toml::from_str(doc).context("parsing taxonomy.toml")?;
        for e in &mut raw.entries {
            if e.app_hints.is_empty()
                && e.title_patterns.is_empty()
                && e.title_prefix_ranges.is_empty()
            {
                bail!(
                    "taxonomy entry {:?} has no app_hints, title_patterns, or title_prefix_ranges (would match every frame)",
                    e.id
                );
            }
            if e.id.trim().is_empty() {
                bail!("taxonomy entry has an empty id");
            }
            e.prefix_ranges = parse_prefix_ranges(&e.title_prefix_ranges)
                .with_context(|| format!("taxonomy entry {:?} title_prefix_ranges", e.id))?;
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

    /// File-order index of an entry id, or `usize::MAX` if absent. The macro grouping pass uses
    /// it as the final, deterministic tie-break when selecting a session's AI anchor (`group`).
    pub fn entry_index(&self, id: &str) -> usize {
        self.entries
            .iter()
            .position(|e| e.id == id)
            .unwrap_or(usize::MAX)
    }

    /// Recognize a tool/meeting from a frame's context, or `None` if nothing matches.
    pub fn recognize(&self, app_hint: Option<&str>, title: Option<&str>) -> Option<Recognized> {
        let app = app_hint.map(normalize_stem);
        let title = title.map(|t| t.to_ascii_lowercase());
        if app.is_none() && title.is_none() {
            return None;
        }
        // The title's first non-whitespace char (its scalar value), for prefix-range matching.
        // ASCII-lowercasing above leaves the spinner glyphs (✳, braille) unchanged.
        let title_prefix: Option<u32> = title
            .as_deref()
            .and_then(|t| t.chars().find(|c| !c.is_whitespace()))
            .map(|c| c as u32);
        // app_hint matches by exact stem (a process name is not a substring of another);
        // title matches by substring OR by a first-glyph prefix range (the window title carries
        // free-form context; the prefix range catches spinner-titled tools, gap #111).
        let app_matches = |hints: &[String]| -> bool {
            match &app {
                Some(a) => hints.iter().any(|h| a == &normalize_stem(h)),
                None => false,
            }
        };
        let title_contains = |patterns: &[String]| -> bool {
            match &title {
                Some(t) => patterns.iter().any(|p| t.contains(p.as_str())),
                None => false,
            }
        };
        let title_prefix_in = |ranges: &[(u32, u32)]| -> bool {
            match title_prefix {
                Some(sc) => ranges.iter().any(|&(lo, hi)| sc >= lo && sc <= hi),
                None => false,
            }
        };
        for e in &self.entries {
            let app_ok = e.app_hints.is_empty() || app_matches(&e.app_hints);
            let title_ok = (e.title_patterns.is_empty() && e.prefix_ranges.is_empty())
                || title_contains(&e.title_patterns)
                || title_prefix_in(&e.prefix_ranges);
            let title_excluded = title.as_ref().is_some_and(|title| {
                e.title_exclude_patterns
                    .iter()
                    .any(|pattern| title.contains(&pattern.to_ascii_lowercase()))
            });
            if app_ok && title_ok && !title_excluded {
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
        // v3 (2026-07-10): claude-code gains title_prefix_ranges (spinner rule, gap #111).
        assert_eq!(t.version, 3);
        // 4 tools + 5 meetings = 9 seed entries (vscode + cursor dropped 2026-07-09).
        assert_eq!(t.entries.len(), 9);
    }

    #[test]
    fn sparkle_prefix_matches_claude_code() {
        let t = Taxonomy::seed();
        // A task-titled Claude Code terminal frame with the ✳ (U+2733) prefix and NO "claude"
        // substring is now recognized (gap #111).
        let r = t
            .recognize(
                Some("WindowsTerminal"),
                Some("\u{2733} Post review comments on PR"),
            )
            .expect("spinner-prefixed claude-code");
        assert_eq!(r.id, "claude-code");
        assert_eq!(r.host, Some(Host::Terminal));
    }

    #[test]
    fn braille_prefix_matches_claude_code() {
        let t = Taxonomy::seed();
        // A braille spinner glyph (U+280B) prefix on a terminal stem, no "claude" substring.
        let r = t
            .recognize(Some("pwsh"), Some("\u{280B} building the workspace"))
            .expect("braille-prefixed claude-code");
        assert_eq!(r.id, "claude-code");
    }

    #[test]
    fn leading_whitespace_before_spinner_tolerated() {
        let t = Taxonomy::seed();
        // The first NON-whitespace char is the spinner.
        assert_eq!(
            t.recognize(Some("wt"), Some("   \u{2733} working"))
                .expect("claude-code")
                .id,
            "claude-code"
        );
    }

    #[test]
    fn plain_shell_title_does_not_match_prefix() {
        let t = Taxonomy::seed();
        // A normal shell prompt (first char 'P') is not a spinner and carries no "claude".
        assert!(t.recognize(Some("pwsh"), Some("PS C:\\repo>")).is_none());
    }

    #[test]
    fn spinner_prefix_confined_to_terminal_stems() {
        let t = Taxonomy::seed();
        // Scope guard (gap #112): a spinner-prefixed title on a BROWSER stem is not claude-code
        // (app mismatch) and not browser-ai (no ai substring; browser-ai has no prefix ranges).
        assert!(t
            .recognize(Some("chrome"), Some("\u{2733} not a terminal"))
            .is_none());
    }

    #[test]
    fn substring_fallback_still_recognizes_claude_code() {
        let t = Taxonomy::seed();
        // An idle terminal titled literally "claude" (no spinner) still matches via substring.
        assert_eq!(
            t.recognize(Some("WindowsTerminal"), Some("claude - repo"))
                .expect("claude-code")
                .id,
            "claude-code"
        );
    }

    #[test]
    fn invalid_prefix_range_rejected_at_parse() {
        // Non-hex spec.
        let bad_hex = r#"
version = 3
[[entry]]
id = "x"
kind = "ai"
app_hints = ["wt"]
title_prefix_ranges = ["zzzz"]
"#;
        assert!(Taxonomy::parse(bad_hex)
            .unwrap_err()
            .to_string()
            .contains("title_prefix_ranges"));
        // lo > hi.
        let bad_order = r#"
version = 3
[[entry]]
id = "x"
kind = "ai"
app_hints = ["wt"]
title_prefix_ranges = ["2800-2700"]
"#;
        assert!(Taxonomy::parse(bad_order).is_err());
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
    fn codex_desktop_app() {
        let t = Taxonomy::seed();
        // Codex is a desktop app on this install (app stem "codex", title "Codex"),
        // recognized on the app stem alone — NOT a terminal.
        let r = t.recognize(Some("codex"), Some("Codex")).expect("codex");
        assert_eq!(r.id, "codex");
        assert_eq!(r.kind, Kind::Ai);
        assert_eq!(r.host, Some(Host::Desktop));
    }

    #[test]
    fn renamed_chatgpt_stem_maps_to_codex_except_classic() {
        let t = Taxonomy::seed();
        assert_eq!(
            t.recognize(Some("chatgpt"), Some("Codex"))
                .expect("renamed codex desktop")
                .id,
            "codex"
        );
        assert!(t
            .recognize(Some("chatgpt"), Some("ChatGPT Classic"))
            .is_none());
    }

    #[test]
    fn codex_stem_does_not_collide_with_a_code_ide_hint() {
        let t = Taxonomy::seed();
        // Regression (2026-07-09): substring matching tagged "codex" as vscode because
        // "codex".contains("code"); exact-stem matching + dropping vscode/cursor fixes it.
        assert_eq!(
            t.recognize(Some("codex"), Some("Codex")).expect("codex").id,
            "codex"
        );
        // A bare "code" stem is no longer recognized at all (vscode/cursor dropped).
        assert!(t
            .recognize(Some("Code"), Some("main.rs - Visual Studio Code"))
            .is_none());
    }

    #[test]
    fn app_hint_matches_by_exact_stem_not_substring() {
        let t = Taxonomy::seed();
        // "powershell" is a claude-code terminal stem; a longer look-alike is not.
        assert_eq!(
            t.recognize(Some("powershell"), Some("claude - repo"))
                .expect("claude-code")
                .id,
            "claude-code"
        );
        assert!(t
            .recognize(Some("powershellx"), Some("claude - repo"))
            .is_none());
        // ".exe" is stripped before comparing stems.
        assert_eq!(
            t.recognize(Some("codex.exe"), Some("Codex"))
                .expect("codex")
                .id,
            "codex"
        );
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
        // Case-insensitive app-stem match.
        assert_eq!(t.recognize(Some("CODEX"), None).expect("codex").id, "codex");
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
            .contains("no app_hints, title_patterns, or title_prefix_ranges"));
    }
}
