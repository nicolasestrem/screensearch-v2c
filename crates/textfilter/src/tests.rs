//! Golden classifier tests over a small, anonymized **synthetic** OCR fixture
//! (no personal screenshot). Deterministic — no DB, no Windows APIs (`docs/0.2.0.md`
//! PR3). The fixture models a typical editor screen: a top menu/toolbar line, a long
//! body line, a short body line, a bottom taskbar clock, and a peeking background
//! window.

use std::collections::HashMap;

use traits::{normalize_text, SuppressReason, TextRole, TextSource, TextSpan};

use super::*;

fn cfg() -> FilterConfig {
    // The spec §8 defaults.
    FilterConfig {
        chrome_suppress_min_seen: 12,
        chrome_protect_min_chars: 48,
        chrome_region_buckets: 8,
    }
}

fn word_span(text: &str, line_index: u32, x: f32, y: f32, w: f32, h: f32) -> TextSpan {
    TextSpan {
        normalized_text: normalize_text(text),
        text: text.to_string(),
        source: TextSource::Ocr,
        role: TextRole::Unknown,
        x,
        y,
        w,
        h,
        line_index,
        is_searchable: true,
        suppress_reason: None,
    }
}

/// Lays a line's words left-to-right at a fixed `y`, with width proportional to the
/// character count (a stand-in for OCR word boxes).
fn line_spans(text: &str, line_index: u32, x_start: f32, y: f32) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    let mut x = x_start;
    let h = 0.02;
    for word in text.split_whitespace() {
        let w = 0.012 * word.chars().count() as f32;
        spans.push(word_span(word, line_index, x, y, w, h));
        x += w + 0.005;
    }
    spans
}

/// The synthetic editor frame. Foreground (editor) window rect = `[0.1, 0.0, 0.8, 0.93]`.
fn fixture() -> (Vec<TextSpan>, [f32; 4]) {
    let rect = [0.1, 0.0, 0.8, 0.93];
    let mut spans = Vec::new();
    // L0: top menu/toolbar — short, near the top edge, inside the window (not interior).
    spans.extend(line_spans("File Edit View Help", 0, 0.12, 0.01));
    // L1: long body line — interior, well over chrome_protect_min_chars.
    spans.extend(line_spans(
        "the quick brown fox jumps over the lazy dog while typing notes",
        1,
        0.20,
        0.40,
    ));
    // L2: short body line — interior (must never be suppressed for repeating).
    spans.extend(line_spans("ok thanks", 2, 0.30, 0.50));
    // L3: taskbar clock — short, bottom band, outside the editor window.
    spans.extend(line_spans("3:47 PM", 3, 0.91, 0.97));
    // L4: peeking background window — outside the editor window, mid-screen.
    spans.extend(line_spans("Inbox 42", 4, 0.92, 0.40));
    (spans, rect)
}

fn input<'a>(
    spans: &'a [TextSpan],
    rect: Option<[f32; 4]>,
    title: Option<&'a str>,
) -> ClassifyInput<'a> {
    ClassifyInput {
        spans,
        target_rect: rect,
        target_window_title: title,
        target_app_hint: Some("editor"),
    }
}

fn roles_of(out: &ClassifyOutput, line_index: u32) -> Vec<TextRole> {
    out.spans
        .iter()
        .filter(|s| s.line_index == line_index)
        .map(|s| s.role)
        .collect()
}

fn observed_sig_for(out: &ClassifyOutput, normalized: &str) -> String {
    out.observed
        .iter()
        .find(|o| o.normalized_text == normalized)
        .unwrap_or_else(|| panic!("no observed signature for {normalized:?}"))
        .signature
        .clone()
}

#[test]
fn default_frame_drops_system_and_background_keeps_content() {
    let (spans, rect) = fixture();
    let catalog: HashMap<String, u32> = HashMap::new();
    let out = classify(&input(&spans, Some(rect), None), &catalog, &cfg());

    // Clock = system, peeking window = background.
    assert!(roles_of(&out, 3).iter().all(|r| *r == TextRole::System));
    assert!(roles_of(&out, 4).iter().all(|r| *r == TextRole::Background));
    // Long body + short interior body = content.
    assert!(roles_of(&out, 1).iter().all(|r| *r == TextRole::Content));
    assert!(roles_of(&out, 2).iter().all(|r| *r == TextRole::Content));
    // Toolbar is a candidate but unseen → kept on first appearance.
    assert!(roles_of(&out, 0).iter().all(|r| *r == TextRole::Content));

    // content_text excludes the clock and the background window; keeps the body.
    assert!(out.content_text.contains("the quick brown fox"));
    assert!(out.content_text.contains("ok thanks"));
    assert!(out.content_text.contains("File Edit View Help"));
    assert!(!out.content_text.contains("3:47"));
    assert!(!out.content_text.contains("Inbox"));

    // suppressed_count = clock(2) + background(2) words.
    assert_eq!(out.suppressed_count, 4);
}

#[test]
fn toolbar_becomes_chrome_at_the_seen_threshold() {
    let (spans, rect) = fixture();
    let empty: HashMap<String, u32> = HashMap::new();
    let first = classify(&input(&spans, Some(rect), None), &empty, &cfg());
    let toolbar_sig = observed_sig_for(&first, "file edit view help");

    // One below threshold (seen 10 → +1 = 11 < 12): still kept.
    let mut below: HashMap<String, u32> = HashMap::new();
    below.insert(toolbar_sig.clone(), 10);
    let out_below = classify(&input(&spans, Some(rect), None), &below, &cfg());
    assert!(roles_of(&out_below, 0)
        .iter()
        .all(|r| *r == TextRole::Content));
    assert!(out_below.content_text.contains("File Edit View Help"));

    // At threshold (seen 11 → +1 = 12 >= 12): suppressed as static chrome.
    let mut at: HashMap<String, u32> = HashMap::new();
    at.insert(toolbar_sig, 11);
    let out_at = classify(&input(&spans, Some(rect), None), &at, &cfg());
    assert!(roles_of(&out_at, 0).iter().all(|r| *r == TextRole::Chrome));
    assert!(out_at
        .spans
        .iter()
        .filter(|s| s.line_index == 0)
        .all(|s| s.suppress_reason == Some(SuppressReason::StaticChrome) && !s.is_searchable));
    assert!(!out_at.content_text.contains("File Edit View Help"));
    // The long body and short interior body survive regardless of the catalog.
    assert!(out_at.content_text.contains("the quick brown fox"));
    assert!(out_at.content_text.contains("ok thanks"));
    assert!(roles_of(&out_at, 1).iter().all(|r| *r == TextRole::Content));
    assert!(roles_of(&out_at, 2).iter().all(|r| *r == TextRole::Content));
}

#[test]
fn short_interior_body_is_never_catalogued() {
    // "ok thanks" is interior, so it must not even produce a signature to suppress.
    let (spans, rect) = fixture();
    let empty: HashMap<String, u32> = HashMap::new();
    let out = classify(&input(&spans, Some(rect), None), &empty, &cfg());
    assert!(
        out.observed
            .iter()
            .all(|o| o.normalized_text != "ok thanks"),
        "interior short body must not be a chrome candidate"
    );
}

#[test]
fn window_title_echoed_as_body_is_excluded() {
    let (mut spans, rect) = fixture();
    // A line that repeats the window title verbatim (e.g. a header echoing the title).
    spans.extend(line_spans("My Project Editor", 5, 0.20, 0.20));
    let catalog: HashMap<String, u32> = HashMap::new();
    let out = classify(
        &input(&spans, Some(rect), Some("My Project Editor")),
        &catalog,
        &cfg(),
    );
    assert!(roles_of(&out, 5).iter().all(|r| *r == TextRole::Chrome));
    assert!(!out.content_text.contains("My Project Editor"));
}

#[test]
fn no_target_rect_never_classifies_background_or_system() {
    // Foreground window on another monitor / unresolved: nothing is suppressed
    // positionally; everything kept (as unknown) and only the catalog can demote.
    let (spans, _rect) = fixture();
    let catalog: HashMap<String, u32> = HashMap::new();
    let out = classify(&input(&spans, None, None), &catalog, &cfg());
    assert!(out
        .spans
        .iter()
        .all(|s| s.role != TextRole::Background && s.role != TextRole::System));
    // The clock and background text are conservatively kept (recoverable, not lost).
    assert!(out.content_text.contains("3:47"));
    assert!(out.content_text.contains("Inbox"));
    assert_eq!(out.suppressed_count, 0);
}

#[test]
fn empty_spans_produce_empty_output() {
    let catalog: HashMap<String, u32> = HashMap::new();
    let out = classify(
        &input(&[], Some([0.0, 0.0, 1.0, 1.0]), None),
        &catalog,
        &cfg(),
    );
    assert!(out.content_text.is_empty());
    assert_eq!(out.suppressed_count, 0);
    assert!(out.observed.is_empty());
    assert!(out.spans.is_empty());
}
