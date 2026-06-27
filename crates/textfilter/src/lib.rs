//! Attention-first text classifier (0.2.x, `03 §3b` / `docs/0.2.0.md` PR3).
//!
//! Takes a frame's OCR/UIA spans (word-level, with normalized `[0,1]` geometry and
//! an OCR `line_index`) and assigns each a [`TextRole`], then derives the filtered
//! **`content_text`** that search, Ask, embeddings, and reports use by default. The
//! goal is to stop static chrome — taskbars, tray/clock, desktop icons, toolbars,
//! menus, sidebars, status bars, repeated app labels — and background-window text
//! from dominating retrieval, while **never** dropping document/editor/terminal/chat
//! body text.
//!
//! ## Purity
//! This crate is pure and deterministic: it depends on `traits` only, performs no
//! I/O, and touches no Windows APIs. The one piece of cross-frame state — the static
//! **chrome catalog** (how often a span signature has been seen) — is injected via
//! the [`ChromeCatalog`] trait, so the store can back it with SQL while the golden
//! tests back it with an in-memory map. The classifier returns the
//! [`ObservedSignature`]s the caller should bump in the catalog after classifying.
//!
//! ## Roles (`03 §3b`)
//! - `system` — taskbar / tray / clock / Start, detected as short text in the bottom
//!   band **outside** the focused window.
//! - `background` — text outside the target/foreground window rect.
//! - `chrome` — repeated app labels (static-chrome catalog) and the window title
//!   echoed as body text.
//! - `content` — body text inside the target window (the default-kept role).
//! - `unknown` — kept when we can't confidently place it (e.g. no target rect) and it
//!   isn't obviously static noise.
//!
//! `content_text` keeps `content` + `unknown` and excludes `system`/`background`/
//! `chrome`. The foreground window **title** is metadata, never appended to the body.
//!
//! ## Top risk: false suppression
//! Wrongly dropping real content is silent data loss, so the classifier is
//! conservative: long, information-rich lines (`>= chrome_protect_min_chars`) are
//! never suppressed for repeating; **all** suppression — `background`/`system`
//! (positional) *and* static-chrome (repetition) — only fires when the target rect is
//! **known**, so with `None` the classifier suppresses nothing and an unknown/wrong
//! rect can only ever *under*-suppress, never silently lose content; and anything
//! dropped is still recoverable via `include_chrome` + the preserved `raw_text`. The
//! caller exposes a per-app suppression-rate metric so over-suppression is observable.

use std::collections::BTreeMap;
use std::collections::HashMap;

use traits::{normalize_text, SuppressReason, TextRole, TextSpan};

/// Field separator inside a chrome signature. A non-printable unit-separator so
/// `app_hint`/`region`/`normalized_text` can never collide by concatenation (e.g.
/// `app="a", text="b|c"` vs `app="a|b", text="c"`).
const SEP: char = '\u{1f}';

/// A line whose top edge is at/below this normalized `y` sits in the bottom
/// taskbar/tray band. Conservative (~the bottom 5%, well under a 48 px taskbar on a
/// 1080p monitor). Internal geometry constant — not a suppression threshold (those
/// are the settings in [`FilterConfig`], `03 §8`). Recorded in `07`.
const SYSTEM_BAND_TOP: f32 = 0.95;

/// Fraction the target rect is inset on each side to define the "interior content"
/// zone. A short line whose centroid is interior is treated as body text and is
/// **not** consulted against the chrome catalog (so short body text that happens to
/// repeat is not suppressed); edge lines (toolbars/sidebars/status bars) are. Internal
/// constant (recorded in `07`).
const CONTENT_INSET: f32 = 0.06;

/// Minimum fraction of a line's area that must fall inside the target rect for the
/// line to count as "inside the focused window". Below this, the line is background
/// (or, in the bottom band, system). Internal constant (recorded in `07`).
const INSIDE_MIN_FRACTION: f32 = 0.5;

/// The configurable suppression thresholds (`03 §8`, mirrored from [`traits::Settings`]
/// `text.*`). Thresholds are settings, never hardcoded (`03 §3b` guardrail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterConfig {
    /// Appearances of a signature (including the current frame) at which it is marked
    /// static chrome and dropped from `content_text` (`text.chrome_suppress_min_seen`).
    pub chrome_suppress_min_seen: u32,
    /// Lines at least this many characters are never suppressed for merely repeating
    /// (`text.chrome_protect_min_chars`).
    pub chrome_protect_min_chars: u32,
    /// N for the N×N `region_bucket` grid over the normalized frame
    /// (`text.chrome_region_buckets`).
    pub chrome_region_buckets: u32,
}

/// Read access to the static-chrome catalog: how many times a signature has been seen
/// in **prior** frames (0 if never). Injected so this crate stays I/O-free — the store
/// backs it with SQL, tests with a `HashMap`.
pub trait ChromeCatalog {
    /// Prior appearances of `signature` (excludes the current frame).
    fn seen_count(&self, signature: &str) -> u32;
}

/// In-memory catalog (golden tests; also handy for callers that pre-load a snapshot).
impl ChromeCatalog for HashMap<String, u32> {
    fn seen_count(&self, signature: &str) -> u32 {
        self.get(signature).copied().unwrap_or(0)
    }
}

/// Inputs to one frame's classification.
pub struct ClassifyInput<'a> {
    /// Word spans in `span_index` (reading) order, each carrying its OCR `line_index`.
    pub spans: &'a [TextSpan],
    /// Normalized `[0,1]` foreground-window rect within this frame, or `None` (other
    /// monitor / minimized / unresolved). With `None` the classifier never emits
    /// `background`/`system` — the safe default.
    pub target_rect: Option<[f32; 4]>,
    /// Foreground window title (carried as metadata; if a line echoes it verbatim that
    /// line is chrome, not body).
    pub target_window_title: Option<&'a str>,
    /// Foreground app/process hint, scopes chrome signatures per app.
    pub target_app_hint: Option<&'a str>,
}

/// A signature the caller should record in the chrome catalog after classifying (one
/// per distinct candidate line in the frame; deduplicated by `signature`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedSignature {
    pub signature: String,
    pub app_hint: Option<String>,
    pub region_bucket: String,
    pub normalized_text: String,
}

/// Result of classifying one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifyOutput {
    /// The input spans with `role` / `is_searchable` / `suppress_reason` assigned, in
    /// the same order as the input.
    pub spans: Vec<TextSpan>,
    /// Filtered default-retrieval text: kept lines (`content` + `unknown`) joined in
    /// reading order. Excludes the window title.
    pub content_text: String,
    /// Number of word spans dropped from `content_text` (the suppression-rate metric
    /// numerator).
    pub suppressed_count: u32,
    /// Candidate signatures to bump in the chrome catalog (deduped per frame).
    pub observed: Vec<ObservedSignature>,
}

/// Classifies one frame's spans into roles and derives `content_text` (`03 §3b`).
pub fn classify(
    input: &ClassifyInput<'_>,
    catalog: &dyn ChromeCatalog,
    config: &FilterConfig,
) -> ClassifyOutput {
    let lines = group_lines(input.spans);
    let title_norm = input
        .target_window_title
        .map(normalize_text)
        .filter(|t| !t.is_empty());
    let protect_min = config.chrome_protect_min_chars as usize;
    let min_seen = config.chrome_suppress_min_seen.max(1);
    let buckets = config.chrome_region_buckets.max(1);

    // Per-line decision, keyed by line_index.
    let mut decisions: HashMap<u32, (TextRole, Option<SuppressReason>)> = HashMap::new();
    let mut observed: Vec<ObservedSignature> = Vec::new();
    let mut seen_sigs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in lines.values() {
        let bbox = line.bbox();
        let (cx, cy) = line.centroid();
        let short = line.normalized.chars().count() < protect_min;
        let inside = input
            .target_rect
            .map(|r| inside_fraction(bbox, r))
            .unwrap_or(0.0);

        // 1. system: short text in the bottom band, outside the focused window. Only
        //    when the focused-window rect is known (so a secondary monitor with no
        //    target rect never has its bottom content mislabeled as taskbar).
        if short
            && line.top_y() >= SYSTEM_BAND_TOP
            && input.target_rect.is_some()
            && inside < INSIDE_MIN_FRACTION
        {
            decisions.insert(
                line.index,
                (TextRole::System, Some(SuppressReason::SystemUi)),
            );
            continue;
        }

        // 2. background: line mostly outside the focused window.
        if input.target_rect.is_some() && inside < INSIDE_MIN_FRACTION {
            decisions.insert(
                line.index,
                (TextRole::Background, Some(SuppressReason::BackgroundWindow)),
            );
            continue;
        }

        // 3. title echoed as body text → chrome (the title is metadata, not content).
        if let Some(ref title) = title_norm {
            if &line.normalized == title {
                decisions.insert(
                    line.index,
                    (TextRole::Chrome, Some(SuppressReason::StaticChrome)),
                );
                continue;
            }
        }

        // 4. static-chrome candidate: short, inside a **known** target rect but not in
        //    its interior content zone (edges = toolbars/sidebars/status bars). Long
        //    lines and interior body text are never catalogued or suppressed for
        //    repeating. Requires a known rect: with no geometry we can't tell a short
        //    toolbar label from short body text, so a rect-less frame never catalogs or
        //    suppresses (the line falls through to `unknown`, kept) — repetition alone
        //    must never drop content we can't place. This keeps the invariant that an
        //    unknown rect can only ever *under*-suppress, never silently lose content.
        if short {
            if let Some(rect) = input.target_rect {
                if !centroid_interior(rect, cx, cy) {
                    let region = region_bucket(cx, cy, buckets);
                    let sig = signature(input.target_app_hint, &region, &line.normalized);
                    if seen_sigs.insert(sig.clone()) {
                        observed.push(ObservedSignature {
                            signature: sig.clone(),
                            app_hint: input.target_app_hint.map(|s| s.to_string()),
                            region_bucket: region,
                            normalized_text: line.normalized.clone(),
                        });
                    }
                    // Suppress once this appearance reaches the threshold.
                    if catalog.seen_count(&sig).saturating_add(1) >= min_seen {
                        decisions.insert(
                            line.index,
                            (TextRole::Chrome, Some(SuppressReason::StaticChrome)),
                        );
                        continue;
                    }
                }
            }
        }

        // 5. default: kept. Content when we know it's inside the target window,
        //    Unknown when we have no rect to place it.
        let role = if input.target_rect.is_some() {
            TextRole::Content
        } else {
            TextRole::Unknown
        };
        decisions.insert(line.index, (role, None));
    }

    // Apply decisions to spans (same order as input), tally suppressed, build content.
    let mut out_spans = Vec::with_capacity(input.spans.len());
    let mut suppressed_count: u32 = 0;
    for span in input.spans {
        let (role, reason) = decisions
            .get(&span.line_index)
            .copied()
            .unwrap_or((TextRole::Unknown, None));
        let kept = matches!(role, TextRole::Content | TextRole::Unknown);
        if !kept {
            suppressed_count += 1;
        }
        out_spans.push(TextSpan {
            role,
            is_searchable: kept,
            suppress_reason: reason,
            ..span.clone()
        });
    }

    let content_text = build_content_text(&lines, &decisions);

    ClassifyOutput {
        spans: out_spans,
        content_text,
        suppressed_count,
        observed,
    }
}

/// Re-applies **only** the static-chrome (repetition) suppression to a frame's already
/// classified `spans`, against a now-warm `catalog`. Positional roles decided at capture
/// (`background`/`system`/`chrome`) are preserved; only currently-kept lines
/// (`content`/`unknown`) can be demoted to `chrome`.
///
/// Unlike [`classify`] this needs **no `target_rect`** — a line's chrome signature is
/// centroid-grid + app based, both derivable from the stored span geometry — and it is
/// **monotonic**: it can only ever suppress *more*, never resurrect content, so re-running
/// it is safe and idempotent. It backs the store's `filter_version` backfill
/// (`docs/0.2.0.md` PR3 follow-up): the live classifier's cold-start window — a repeated
/// label is kept until its signature crosses `chrome_suppress_min_seen` — is retroactively
/// cleaned once the catalog has learned the label, so old frames stop surfacing app/nav
/// chrome in default search. The catalog already counts each frame's appearance (bumped at
/// capture), so the test is `seen_count >= min_seen` — **no `+1`** (that would double-count
/// this frame, unlike the live path where the bump happens after classify).
pub fn reconcile(
    spans: &[TextSpan],
    target_app_hint: Option<&str>,
    catalog: &dyn ChromeCatalog,
    config: &FilterConfig,
) -> ClassifyOutput {
    let lines = group_lines(spans);
    let protect_min = config.chrome_protect_min_chars as usize;
    let min_seen = config.chrome_suppress_min_seen.max(1);
    let buckets = config.chrome_region_buckets.max(1);

    // Line indices to newly demote (content/unknown → chrome).
    let mut newly_chrome: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for line in lines.values() {
        // Preserve positional suppression already baked in at capture.
        if !matches!(line.role, TextRole::Content | TextRole::Unknown) {
            continue;
        }
        // Long, information-rich lines are never suppressed for repeating.
        if line.normalized.chars().count() >= protect_min {
            continue;
        }
        let (cx, cy) = line.centroid();
        let sig = signature(
            target_app_hint,
            &region_bucket(cx, cy, buckets),
            &line.normalized,
        );
        if catalog.seen_count(&sig) >= min_seen {
            newly_chrome.insert(line.index);
        }
    }

    // Apply to spans (same order as input), tally suppressed, rebuild content.
    let mut out_spans = Vec::with_capacity(spans.len());
    let mut suppressed_count: u32 = 0;
    for span in spans {
        let (role, reason, searchable) = if newly_chrome.contains(&span.line_index) {
            (TextRole::Chrome, Some(SuppressReason::StaticChrome), false)
        } else {
            let kept = matches!(span.role, TextRole::Content | TextRole::Unknown);
            (span.role, span.suppress_reason, kept)
        };
        if !searchable {
            suppressed_count += 1;
        }
        out_spans.push(TextSpan {
            role,
            is_searchable: searchable,
            suppress_reason: reason,
            ..span.clone()
        });
    }

    let content_text = lines
        .values()
        .filter(|l| {
            matches!(l.role, TextRole::Content | TextRole::Unknown)
                && !newly_chrome.contains(&l.index)
        })
        .map(|l| l.text())
        .collect::<Vec<_>>()
        .join("\n");

    ClassifyOutput {
        spans: out_spans,
        content_text,
        suppressed_count,
        observed: Vec::new(),
    }
}

/// One OCR line aggregated from its word spans.
struct LineAcc {
    index: u32,
    words: Vec<String>,
    normalized: String,
    /// The role already assigned to this line (from the first span's stored role).
    /// `classify` ignores it — every input span is `Unknown` there — but [`reconcile`]
    /// reads it to preserve positional suppression decided at capture.
    role: TextRole,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl LineAcc {
    fn bbox(&self) -> [f32; 4] {
        [
            self.x0,
            self.y0,
            (self.x1 - self.x0).max(0.0),
            (self.y1 - self.y0).max(0.0),
        ]
    }
    fn centroid(&self) -> (f32, f32) {
        ((self.x0 + self.x1) * 0.5, (self.y0 + self.y1) * 0.5)
    }
    fn top_y(&self) -> f32 {
        self.y0
    }
    fn text(&self) -> String {
        self.words.join(" ")
    }
}

/// Groups word spans into lines by `line_index` (sorted ascending = reading order),
/// computing each line's union bbox and normalized text.
fn group_lines(spans: &[TextSpan]) -> BTreeMap<u32, LineAcc> {
    let mut lines: BTreeMap<u32, LineAcc> = BTreeMap::new();
    for span in spans {
        let entry = lines.entry(span.line_index).or_insert_with(|| LineAcc {
            index: span.line_index,
            words: Vec::new(),
            normalized: String::new(),
            role: span.role,
            x0: f32::INFINITY,
            y0: f32::INFINITY,
            x1: f32::NEG_INFINITY,
            y1: f32::NEG_INFINITY,
        });
        entry.words.push(span.text.clone());
        entry.x0 = entry.x0.min(span.x);
        entry.y0 = entry.y0.min(span.y);
        entry.x1 = entry.x1.max(span.x + span.w);
        entry.y1 = entry.y1.max(span.y + span.h);
    }
    // Derive the canonical normalized line text from the joined words.
    for line in lines.values_mut() {
        line.normalized = normalize_text(&line.text());
        if !line.x0.is_finite() {
            line.x0 = 0.0;
            line.y0 = 0.0;
            line.x1 = 0.0;
            line.y1 = 0.0;
        }
    }
    lines
}

/// Joins the kept lines (`content`/`unknown`) in reading order into `content_text`.
fn build_content_text(
    lines: &BTreeMap<u32, LineAcc>,
    decisions: &HashMap<u32, (TextRole, Option<SuppressReason>)>,
) -> String {
    let mut kept: Vec<String> = Vec::new();
    for line in lines.values() {
        let role = decisions
            .get(&line.index)
            .map(|(r, _)| *r)
            .unwrap_or(TextRole::Unknown);
        if matches!(role, TextRole::Content | TextRole::Unknown) {
            kept.push(line.text());
        }
    }
    kept.join("\n")
}

/// `app_hint + region_bucket + normalized_text`, separator-delimited (`03 §3b`).
fn signature(app_hint: Option<&str>, region: &str, normalized: &str) -> String {
    let app = app_hint.unwrap_or("");
    format!("{app}{SEP}{region}{SEP}{normalized}")
}

/// `"row,col"` cell of the N×N grid for a line centroid (clamped to `[0, n)`).
fn region_bucket(cx: f32, cy: f32, n: u32) -> String {
    let nf = n as f32;
    let col = ((cx.clamp(0.0, 1.0) * nf) as u32).min(n - 1);
    let row = ((cy.clamp(0.0, 1.0) * nf) as u32).min(n - 1);
    format!("{row},{col}")
}

/// Fraction of `line`'s area covered by `rect` (centroid containment for a zero-area
/// line). Both are `[x, y, w, h]`.
fn inside_fraction(line: [f32; 4], rect: [f32; 4]) -> f32 {
    let area = line[2] * line[3];
    if area <= 0.0 {
        let cx = line[0];
        let cy = line[1];
        let inside =
            cx >= rect[0] && cx <= rect[0] + rect[2] && cy >= rect[1] && cy <= rect[1] + rect[3];
        return if inside { 1.0 } else { 0.0 };
    }
    let ix0 = line[0].max(rect[0]);
    let iy0 = line[1].max(rect[1]);
    let ix1 = (line[0] + line[2]).min(rect[0] + rect[2]);
    let iy1 = (line[1] + line[3]).min(rect[1] + rect[3]);
    let iw = (ix1 - ix0).max(0.0);
    let ih = (iy1 - iy0).max(0.0);
    (iw * ih) / area
}

/// Whether a centroid is in the inset interior of `rect`.
fn centroid_interior(rect: [f32; 4], cx: f32, cy: f32) -> bool {
    let ix0 = rect[0] + CONTENT_INSET * rect[2];
    let ix1 = rect[0] + rect[2] - CONTENT_INSET * rect[2];
    let iy0 = rect[1] + CONTENT_INSET * rect[3];
    let iy1 = rect[1] + rect[3] - CONTENT_INSET * rect[3];
    cx >= ix0 && cx <= ix1 && cy >= iy0 && cy <= iy1
}

#[cfg(test)]
mod tests;
