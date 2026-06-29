//! The COM **MTA** worker thread that owns the `IUIAutomation` instance and services
//! recognize requests. UI Automation is free-threaded and Microsoft recommends an MTA, so
//! (unlike the OCR STA worker) this initializes `COINIT_MULTITHREADED`. All UIA calls run
//! here; the async side ([`crate::UiaTextProvider::recognize`]) only sends plain `Send`
//! data over a channel, so no COM pointer ever crosses a thread boundary.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use tokio::sync::oneshot;
use traits::{normalize_text, OcrResult, TextRole, TextSource, TextSpan};

use windows::core::Interface;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationValuePattern, UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic};

use crate::{classify, geometry, monitors, UiaBudget, CONFIDENCE_UNKNOWN};

/// A recognize request handed to the MTA worker. Carries only `Send` plain data — never
/// the captured pixels (UIA reads the live tree, not the bitmap) and never a COM pointer.
pub(crate) struct Request {
    /// Captured monitor pixel size (== the monitor's resolution) for `[0,1]` mapping.
    pub width: u32,
    pub height: u32,
    /// Normalized `[0,1]` target-window rect within the captured monitor (PR3), used to
    /// drop spans that fall outside the foreground window. `None` ⇒ no containment filter.
    pub target_rect: Option<[f32; 4]>,
    pub monitor_index: u32,
    /// Foreground window handle (as `i64`) at capture time, or `None` if capture couldn't
    /// resolve it. The worker compares it to `GetForegroundWindow` at recognition time and
    /// bails (→ OCR) on a mismatch, so a focus change between capture and recognize can't
    /// attribute another window's text to this frame (`07` #48).
    pub foreground_hwnd: Option<i64>,
    /// A `tokio` oneshot so the async caller awaits the reply directly (no `spawn_blocking`):
    /// on a caller-side timeout the receiver is dropped and `send` below simply fails.
    pub resp: oneshot::Sender<Result<OcrResult>>,
}

/// Hard caps so a pathological accessibility tree can't blow the latency budget or memory.
const MAX_NODES: u32 = 4000;
const MAX_DEPTH: u32 = 40;
const MAX_SPANS: usize = 10_000;
/// Upper bound on *pending* (pushed-but-not-yet-visited) elements. `MAX_NODES` only bounds
/// nodes popped; without this a single node with a huge sibling fan-out (virtualized
/// lists/grids commonly expose tens of thousands) — or a malformed provider returning a
/// *cyclic* sibling chain — could push unboundedly. Paired with the per-descent deadline
/// check below, this keeps the walk inside its latency + memory budget for any tree shape.
const MAX_STACK: usize = 16_000;

/// Rate-limit the "walk exceeded budget" warning: only every Nth occurrence is logged so a
/// sustained-pressure session doesn't spam the log one line per frame.
static OVER_BUDGET_WARNS: AtomicU64 = AtomicU64::new(0);
const WARN_EVERY: u64 = 64;

/// Worker entry point: init COM (MTA), create the automation object once, then service
/// requests until the channel closes (the provider was dropped). `in_flight` is shared with
/// the async provider: this worker — the sole authority — sets it on receiving a request and
/// clears it (RAII) when the walk returns, so `recognize` can skip a frame to OCR whenever a
/// walk is already running, even after the caller's hard timeout abandoned the future.
pub(crate) fn worker_main(
    rx: mpsc::Receiver<Request>,
    ready: mpsc::Sender<Result<()>>,
    budget: UiaBudget,
    in_flight: Arc<AtomicBool>,
) {
    // SAFETY: COM apartment init for the dedicated UIA worker thread. UIA prefers MTA.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        let _ = ready.send(Err(anyhow::anyhow!("CoInitializeEx(MTA) failed: {e}")));
        return;
    }
    // RAII: pair the successful `CoInitializeEx` with `CoUninitialize` on *every* exit path —
    // the `CoCreateInstance` failure return, the normal channel-closed exit, and an
    // unexpected panic in `read_foreground` (which would otherwise skip a manual call).
    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: balances the CoInitializeEx above; runs once on thread teardown.
            unsafe { CoUninitialize() };
        }
    }
    let _com = ComApartment;

    // SAFETY: standard COM object creation; `CUIAutomation` is the documented CLSID.
    let automation: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) } {
            Ok(a) => a,
            Err(e) => {
                let _ = ready.send(Err(anyhow::anyhow!(
                    "CoCreateInstance(CUIAutomation) failed: {e}"
                )));
                return; // `_com` drops → CoUninitialize.
            }
        };

    // Best-effort per-call timeouts (IUIAutomation2, Windows 8+, so always present on our
    // Win11 target). A single cross-process UIA call to an unresponsive/hung provider would
    // otherwise block the worker indefinitely — the in-walk deadline only fires *between*
    // calls, not during one. Bounding both to the (clamped) latency budget keeps one hung
    // call from wedging the worker; the call just errors and that element is skipped.
    if let Ok(a2) = automation.cast::<IUIAutomation2>() {
        let ms = budget.latency_ms.clamp(1, u32::MAX as u64) as u32;
        // SAFETY: setters on the live automation object on the COM thread; failure is benign.
        unsafe {
            let _ = a2.SetConnectionTimeout(ms);
            let _ = a2.SetTransactionTimeout(ms);
        }
    }
    let _ = ready.send(Ok(()));

    // RAII flag-clearer so `in_flight` is reset on every exit path of a walk — including a
    // panic in `read_foreground` — never leaving the guard stuck "busy" (which would wedge
    // every future frame onto the OCR fallback).
    struct InFlight<'a>(&'a AtomicBool);
    impl Drop for InFlight<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }

    while let Ok(req) = rx.recv() {
        in_flight.store(true, Ordering::Release);
        let _guard = InFlight(&in_flight);
        let result = read_foreground(&automation, &req, &budget);
        let _ = req.resp.send(result);
        // `_guard` drops → in_flight = false, admitting the next frame.
    }

    // `_com` drops here (channel closed, provider gone) → CoUninitialize.
}

/// Reads the foreground window's accessibility text into an [`OcrResult`] tagged
/// `engine = "uia"` with `source = TextSource::Uia` spans. Returns `Err` (so the composite
/// falls back to OCR) when this frame's monitor doesn't hold the foreground window
/// (`target_rect` is `None`), there is no usable foreground element, the latency budget is
/// exceeded, or the yield is below `min_text_chars`.
fn read_foreground(
    automation: &IUIAutomation,
    req: &Request,
    budget: &UiaBudget,
) -> Result<OcrResult> {
    // On a multi-monitor capture, `target_rect` is `Some` only for the frame whose monitor
    // holds the foreground window (capture's `normalize_window_rect` returns `None` for every
    // other monitor — only the display whose region contains the window's centre gets a rect).
    // UIA always reads `GetForegroundWindow`, so with no target rect we cannot confirm the
    // foreground window is even on *this* frame's monitor; proceeding would tag a background
    // monitor's frame with the foreground window's text — silent data corruption, and UIA is
    // default-ON. Treat `None` as a UIA miss → OCR fallback, which reads this monitor's pixels.
    let Some(target_rect) = req.target_rect else {
        bail!("no target_rect (foreground window not on this monitor) — fall back to OCR");
    };

    // The foreground window "now"; capture→recognize are sequential, so this is the
    // captured one. Skip our own window class of failures the same way capture's gate does.
    // SAFETY: plain Win32 queries on this thread.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        bail!("no foreground window");
    }
    // SAFETY: IsIconic on a valid HWND.
    if unsafe { IsIconic(hwnd) }.as_bool() {
        bail!("foreground window minimized");
    }
    // Confirm the foreground window is still the one capture saw. A focus change between the
    // WGC frame and this recognize call would otherwise read a *different* window's text into
    // this frame — and `within_target` can't catch it when the new window overlaps the old
    // (captured) rect. Mismatch → bail → OCR for this frame. (`None` ⇒ capture couldn't record
    // a handle; the rect-containment check below remains the guard.)
    if let Some(captured) = req.foreground_hwnd {
        if hwnd.0 as isize as i64 != captured {
            bail!("foreground window changed since capture — fall back to OCR");
        }
    }

    // SAFETY: ElementFromHandle on the foreground HWND; returns Err if it has no element.
    let root = unsafe { automation.ElementFromHandle(hwnd) }?;
    // SAFETY: ControlViewWalker is a property accessor returning the shared control-view
    // walker. Control view (not raw view) is the load fix: a Chromium/Electron raw tree
    // exposes one element per inline text-run, so a long page or a large grid (e.g. the
    // qBittorrent web UI) explodes into tens of thousands of nodes — each a synchronous
    // cross-process call that freezes the target's UI thread. Control view collapses those
    // to the content/control elements that actually carry text, slashing node count while
    // preserving the text we extract (Name / TextPattern visible ranges live on them).
    let walker = unsafe { automation.ControlViewWalker() }?;

    let mon_origin = monitors::monitor_origin(req.monitor_index).unwrap_or((0, 0));
    let start = Instant::now();
    let deadline = start + Duration::from_millis(budget.latency_ms.max(1));

    let mut spans: Vec<TextSpan> = Vec::new();
    let mut text = String::new();
    let mut line_index: u32 = 0;

    // Iterative DFS with an explicit stack (no recursion, no `.await`), bounded by node
    // count, depth, span count, and the soft latency deadline checked every node.
    let mut stack: Vec<(IUIAutomationElement, u32)> = vec![(root, 0)];
    let mut nodes: u32 = 0;
    while let Some((elem, depth)) = stack.pop() {
        if nodes >= MAX_NODES || spans.len() >= MAX_SPANS || Instant::now() >= deadline {
            break;
        }
        nodes += 1;

        // Any property read can fail on a transient element; treat failures as "skip this
        // element" rather than aborting the whole walk.
        // SAFETY: property accessors on a live element on the COM thread.
        let control_type = unsafe { elem.CurrentControlType() }
            .map(|c| c.0)
            .unwrap_or(0);
        let is_password = unsafe { elem.CurrentIsPassword() }
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let is_offscreen = unsafe { elem.CurrentIsOffscreen() }
            .map(|b| b.as_bool())
            .unwrap_or(false);

        if classify::should_emit(control_type, is_password, is_offscreen) {
            if let Some(raw) = extract_text(&elem) {
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    // SAFETY: bounding rect accessor; on failure the span gets a zero box.
                    let (x, y, w, h) = match unsafe { elem.CurrentBoundingRectangle() } {
                        Ok(r) => geometry::normalize_screen_rect(
                            r.left,
                            r.top,
                            r.right,
                            r.bottom,
                            mon_origin,
                            (req.width, req.height),
                        ),
                        Err(_) => (0.0, 0.0, 0.0, 0.0),
                    };
                    if within_target(target_rect, x, y, w, h) {
                        let (words, next) = classify::split_words(trimmed, line_index);
                        for (li, word) in words {
                            spans.push(TextSpan {
                                normalized_text: normalize_text(&word),
                                text: word,
                                source: TextSource::Uia,
                                role: TextRole::Unknown,
                                x,
                                y,
                                w,
                                h,
                                line_index: li,
                                is_searchable: true,
                                suppress_reason: None,
                            });
                        }
                        if next > line_index {
                            text.push_str(trimmed);
                            text.push('\n');
                            line_index = next;
                        }
                    }
                }
            }
        }

        // Descend: push every child (bounded depth). Compute each next sibling before
        // moving the current child onto the stack so no element is cloned. The caps are
        // re-checked *inside* this loop: a single node can have a massive sibling fan-out,
        // and a malformed provider can return a cyclic sibling chain (`GetNextSiblingElement`
        // never yielding `None`), so the once-per-pop outer checks alone don't bound it. The
        // deadline check here is what stops a cycle from spinning the COM thread forever.
        if depth < MAX_DEPTH {
            // SAFETY: tree-walk accessors; Err just means "no (further) child".
            if let Ok(first) = unsafe { walker.GetFirstChildElement(&elem) } {
                let mut child = first;
                loop {
                    if stack.len() >= MAX_STACK || Instant::now() >= deadline {
                        break;
                    }
                    let next = unsafe { walker.GetNextSiblingElement(&child) }.ok();
                    stack.push((child, depth + 1));
                    match next {
                        Some(n) => child = n,
                        None => break,
                    }
                }
            }
        }
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    tracing::debug!(nodes, spans = spans.len(), elapsed_ms, "uia walk complete");
    // A walk that consumed its full latency budget was almost certainly truncated mid-tree
    // (the soft deadline only fires between calls), i.e. the target app is heavy. Surface it
    // — rate-limited — so sustained pressure is visible in the log without per-frame spam.
    if elapsed_ms >= budget.latency_ms
        && OVER_BUDGET_WARNS.fetch_add(1, Ordering::Relaxed) % WARN_EVERY == 0
    {
        tracing::warn!(
            nodes,
            elapsed_ms,
            budget_ms = budget.latency_ms,
            warn_every = WARN_EVERY,
            "uia walk hit its latency budget; target app under load (rate-limited)"
        );
    }

    let text = text.trim_end().to_string();
    let chars = text.chars().count();
    if chars < budget.min_text_chars {
        bail!(
            "uia thin yield ({chars} chars < {} min)",
            budget.min_text_chars
        );
    }
    Ok(OcrResult {
        text,
        mean_confidence: CONFIDENCE_UNKNOWN,
        engine: "uia".to_string(),
        spans,
    })
}

/// Extracts an element's text via the priority ladder: `TextPattern` **visible** ranges
/// (documents/editors — viewport text only, never the scrolled-off document) → `ValuePattern`
/// current value (inputs) → `Name` (labels, buttons, list items). Returns `None` when the
/// element exposes no non-empty text.
fn extract_text(elem: &IUIAutomationElement) -> Option<String> {
    // GetCurrentPattern returns Err when the pattern is unsupported (windows-rs maps the
    // documented S_OK+NULL result to E_POINTER), so each `if let Ok` cleanly skips.
    // SAFETY: pattern/text accessors on a live element on the COM thread.
    unsafe {
        if let Ok(unknown) = elem.GetCurrentPattern(UIA_TextPatternId) {
            if let Ok(text_pattern) = unknown.cast::<IUIAutomationTextPattern>() {
                // Only the *visible* ranges, not `DocumentRange().GetText(-1)`: the document
                // range returns the provider's whole document including scrolled-off text that
                // was never in the captured frame, which both breaks OCR's "only what was
                // visible" parity and (being a large yield) would wrongly suppress the OCR
                // fallback. `GetVisibleRanges` returns just what's in the viewport (`07` #48).
                if let Ok(ranges) = text_pattern.GetVisibleRanges() {
                    if let Ok(len) = ranges.Length() {
                        let mut acc = String::new();
                        for i in 0..len {
                            if let Ok(range) = ranges.GetElement(i) {
                                if let Ok(bstr) = range.GetText(-1) {
                                    let part = bstr.to_string();
                                    if !part.trim().is_empty() {
                                        if !acc.is_empty() {
                                            acc.push('\n');
                                        }
                                        acc.push_str(&part);
                                    }
                                }
                            }
                        }
                        if !acc.trim().is_empty() {
                            return Some(acc);
                        }
                    }
                }
            }
        }
        if let Ok(unknown) = elem.GetCurrentPattern(UIA_ValuePatternId) {
            if let Ok(value_pattern) = unknown.cast::<IUIAutomationValuePattern>() {
                if let Ok(bstr) = value_pattern.CurrentValue() {
                    let s = bstr.to_string();
                    if !s.trim().is_empty() {
                        return Some(s);
                    }
                }
            }
        }
        if let Ok(bstr) = elem.CurrentName() {
            let s = bstr.to_string();
            if !s.trim().is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// Whether a normalized span box is within the target-window rect (by center). The caller
/// (`read_foreground`) has already bailed when the rect is unknown, so this always receives a
/// concrete rect — a span whose centre falls outside the foreground window is dropped.
fn within_target(target: [f32; 4], x: f32, y: f32, w: f32, h: f32) -> bool {
    let [tx, ty, tw, th] = target;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    cx >= tx && cx <= tx + tw && cy >= ty && cy <= ty + th
}

#[cfg(test)]
mod tests {
    use super::within_target;

    #[test]
    fn within_target_filters_by_center() {
        let target = [0.25, 0.25, 0.5, 0.5]; // centered half-screen window
        assert!(within_target(target, 0.4, 0.4, 0.1, 0.1), "center inside");
        assert!(!within_target(target, 0.8, 0.8, 0.1, 0.1), "center outside");
    }
}
