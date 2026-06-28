//! The COM **MTA** worker thread that owns the `IUIAutomation` instance and services
//! recognize requests. UI Automation is free-threaded and Microsoft recommends an MTA, so
//! (unlike the OCR STA worker) this initializes `COINIT_MULTITHREADED`. All UIA calls run
//! here; the async side ([`crate::UiaTextProvider::recognize`]) only sends plain `Send`
//! data over a channel, so no COM pointer ever crosses a thread boundary.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use traits::{normalize_text, OcrResult, TextRole, TextSource, TextSpan};

use windows::core::Interface;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
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
    pub resp: mpsc::Sender<Result<OcrResult>>,
}

/// Hard caps so a pathological accessibility tree can't blow the latency budget or memory.
const MAX_NODES: u32 = 4000;
const MAX_DEPTH: u32 = 40;
const MAX_SPANS: usize = 10_000;

/// Worker entry point: init COM (MTA), create the automation object once, then service
/// requests until the channel closes (the provider was dropped).
pub(crate) fn worker_main(
    rx: mpsc::Receiver<Request>,
    ready: mpsc::Sender<Result<()>>,
    budget: UiaBudget,
) {
    // SAFETY: COM apartment init for the dedicated UIA worker thread. UIA prefers MTA.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        let _ = ready.send(Err(anyhow::anyhow!("CoInitializeEx(MTA) failed: {e}")));
        return;
    }
    // SAFETY: standard COM object creation; `CUIAutomation` is the documented CLSID.
    let automation: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) } {
            Ok(a) => {
                let _ = ready.send(Ok(()));
                a
            }
            Err(e) => {
                let _ = ready.send(Err(anyhow::anyhow!(
                    "CoCreateInstance(CUIAutomation) failed: {e}"
                )));
                // SAFETY: pair every successful CoInitializeEx with CoUninitialize.
                unsafe { CoUninitialize() };
                return;
            }
        };

    while let Ok(req) = rx.recv() {
        let result = read_foreground(&automation, &req, &budget);
        let _ = req.resp.send(result);
    }

    // SAFETY: channel closed (provider dropped) — tear down the apartment.
    unsafe { CoUninitialize() };
}

/// Reads the foreground window's accessibility text into an [`OcrResult`] tagged
/// `engine = "uia"` with `source = TextSource::Uia` spans. Returns `Err` (so the composite
/// falls back to OCR) when there is no usable foreground element, the latency budget is
/// exceeded, or the yield is below `min_text_chars`.
fn read_foreground(
    automation: &IUIAutomation,
    req: &Request,
    budget: &UiaBudget,
) -> Result<OcrResult> {
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

    // SAFETY: ElementFromHandle on the foreground HWND; returns Err if it has no element.
    let root = unsafe { automation.ElementFromHandle(hwnd) }?;
    // SAFETY: RawViewWalker is a property accessor returning the shared raw-view walker.
    let walker = unsafe { automation.RawViewWalker() }?;

    let mon_origin = monitors::monitor_origin(req.monitor_index).unwrap_or((0, 0));
    let deadline = Instant::now() + Duration::from_millis(budget.latency_ms.max(1));

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
                    if within_target(req.target_rect, x, y, w, h) {
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
        // moving the current child onto the stack so no element is cloned.
        if depth < MAX_DEPTH {
            // SAFETY: tree-walk accessors; Err just means "no (further) child".
            if let Ok(first) = unsafe { walker.GetFirstChildElement(&elem) } {
                let mut child = first;
                loop {
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

/// Extracts an element's text via the priority ladder: `TextPattern` document range
/// (documents/editors) → `ValuePattern` current value (inputs) → `Name` (labels, buttons,
/// list items). Returns `None` when the element exposes no non-empty text.
fn extract_text(elem: &IUIAutomationElement) -> Option<String> {
    // GetCurrentPattern returns Err when the pattern is unsupported (windows-rs maps the
    // documented S_OK+NULL result to E_POINTER), so each `if let Ok` cleanly skips.
    // SAFETY: pattern/text accessors on a live element on the COM thread.
    unsafe {
        if let Ok(unknown) = elem.GetCurrentPattern(UIA_TextPatternId) {
            if let Ok(text_pattern) = unknown.cast::<IUIAutomationTextPattern>() {
                if let Ok(range) = text_pattern.DocumentRange() {
                    if let Ok(bstr) = range.GetText(-1) {
                        let s = bstr.to_string();
                        if !s.trim().is_empty() {
                            return Some(s);
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

/// Whether a normalized span box is within the target-window rect (by center). With no
/// known target rect (`None`) every span is kept — the safe default (PR3 then classifies).
fn within_target(target: Option<[f32; 4]>, x: f32, y: f32, w: f32, h: f32) -> bool {
    match target {
        None => true,
        Some([tx, ty, tw, th]) => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            cx >= tx && cx <= tx + tw && cy >= ty && cy <= ty + th
        }
    }
}

#[cfg(test)]
mod tests {
    use super::within_target;

    #[test]
    fn within_target_keeps_everything_when_rect_unknown() {
        assert!(within_target(None, 0.9, 0.9, 0.05, 0.05));
    }

    #[test]
    fn within_target_filters_by_center() {
        let target = Some([0.25, 0.25, 0.5, 0.5]); // centered half-screen window
        assert!(within_target(target, 0.4, 0.4, 0.1, 0.1), "center inside");
        assert!(!within_target(target, 0.8, 0.8, 0.1, 0.1), "center outside");
    }
}
