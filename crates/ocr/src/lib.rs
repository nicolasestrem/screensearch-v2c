//! `ocr` — [`OcrProvider`] via WinRT `Media.Ocr` on a COM **STA** worker (`03 §3`).
//!
//! Native, no model download (~70–80 ms/frame, `01 §5`). WinRT OCR is COM
//! STA-bound: every call runs on the one thread that initialized COM (`01 §6`), so
//! [`WinRtOcr`] owns a dedicated STA worker thread and bridges to async over
//! channels. OCR runs on the **full-resolution** frame before any JPEG resize
//! (`03 §8`).
//!
//! **Confidence:** WinRT `Media.Ocr` exposes no per-word/line confidence (`OcrWord`
//! has only `Text` + `BoundingRect`). Per the user decision (2026-06-21,
//! `06`/`07`), [`OcrResult::mean_confidence`] is set to the sentinel
//! [`CONFIDENCE_UNKNOWN`] (`-1.0`) for WinRT rows — "engine provided none", never a
//! fabricated score.
//!
//! Windows-only by design — no cross-platform fallback (`04` guardrails).

use std::sync::mpsc;
use std::sync::Mutex;

use async_trait::async_trait;
use traits::{
    normalize_text, CapturedFrame, OcrProvider, OcrResult, Result, TextRole, TextSource, TextSpan,
};

use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

/// Sentinel `mean_confidence` for OCR rows: WinRT `Media.Ocr` provides no
/// confidence, so we record "unknown" rather than inventing a number (`06`/`07`).
pub const CONFIDENCE_UNKNOWN: f32 = -1.0;

/// One OCR request handed to the STA worker. Carries owned **BGRA** bytes (already
/// converted on the blocking pool, Send) so the STA worker does no pixel work and no
/// WinRT object ever crosses the thread boundary.
struct Request {
    bgra: Vec<u8>,
    width: u32,
    height: u32,
    resp: mpsc::Sender<Result<OcrResult>>,
}

/// WinRT OCR provider backed by a dedicated COM STA worker thread.
///
/// `Mutex<Sender>` makes the provider `Sync` (the trait requires it); the lock is
/// only ever held to clone the sender, never across the OCR call.
pub struct WinRtOcr {
    tx: Mutex<mpsc::Sender<Request>>,
}

impl WinRtOcr {
    /// Spawns the STA worker, initializes COM, and creates the `OcrEngine` once.
    /// Returns an error if no OCR recognizer language is installed (so the caller
    /// can reflect it in readiness) — never a silent no-op.
    pub fn spawn() -> Result<Self> {
        let (tx, rx) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

        std::thread::Builder::new()
            .name("winrt-ocr-sta".to_string())
            .spawn(move || worker_main(rx, ready_tx))
            .map_err(|e| anyhow::anyhow!("failed to spawn OCR STA thread: {e}"))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { tx: Mutex::new(tx) }),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(anyhow::anyhow!("OCR worker exited during init: {e}")),
        }
    }
}

#[async_trait]
impl OcrProvider for WinRtOcr {
    async fn recognize(&self, frame: &CapturedFrame) -> Result<OcrResult> {
        // Clone the Arc (cheap); the single full-frame copy + RGBA→BGRA swap happens
        // on the blocking pool — never on the async executor or the STA worker.
        let pixels = frame.pixels.clone();
        let (width, height) = (frame.width, frame.height);
        let tx = self.tx.lock().expect("ocr sender poisoned").clone();

        tokio::task::spawn_blocking(move || -> Result<OcrResult> {
            // WinRT OCR wants BGRA8; the capture pipeline produces RGBA8.
            let mut bgra = pixels.as_raw().clone();
            for px in bgra.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            let (resp_tx, resp_rx) = mpsc::channel();
            tx.send(Request {
                bgra,
                width,
                height,
                resp: resp_tx,
            })
            .map_err(|_| anyhow::anyhow!("OCR worker thread is gone"))?;
            resp_rx
                .recv()
                .map_err(|_| anyhow::anyhow!("OCR worker dropped the response"))?
        })
        .await?
    }
}

/// The STA worker thread body: init COM (apartment-threaded), build the engine,
/// then service requests until the channel closes.
fn worker_main(rx: mpsc::Receiver<Request>, ready: mpsc::Sender<Result<()>>) {
    // SAFETY: standard COM apartment init for the WinRT OCR thread (`01 §6`).
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
        let _ = ready.send(Err(anyhow::anyhow!("CoInitializeEx failed: {e}")));
        return;
    }

    let engine = match create_engine() {
        Ok(e) => {
            let _ = ready.send(Ok(()));
            e
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            // SAFETY: pair every successful CoInitializeEx with CoUninitialize.
            unsafe { CoUninitialize() };
            return;
        }
    };

    while let Ok(req) = rx.recv() {
        let result = recognize_blocking(&engine, &req.bgra, req.width, req.height);
        let _ = req.resp.send(result);
    }

    // SAFETY: channel closed (provider dropped) — tear down the apartment.
    unsafe { CoUninitialize() };
}

/// Creates an `OcrEngine` from the user's profile languages, falling back to the
/// first installed recognizer language. Errors if none are installed.
fn create_engine() -> Result<OcrEngine> {
    let available = OcrEngine::AvailableRecognizerLanguages()?;
    if available.Size()? == 0 {
        anyhow::bail!("no OCR recognizer languages are installed");
    }
    match OcrEngine::TryCreateFromUserProfileLanguages() {
        Ok(engine) => Ok(engine),
        Err(_) => {
            let lang: Language = available.GetAt(0)?;
            Ok(OcrEngine::TryCreateFromLanguage(&lang)?)
        }
    }
}

/// Normalizes a pixel-space bounding box to `[0,1]` against the frame size, origin
/// top-left (`03 §3b`). Clamps so the box stays inside the frame and `x + w <= 1`,
/// `y + h <= 1`; a zero-area frame yields a zero box. Pure (no WinRT) so it is
/// unit-tested without a live recognizer.
fn normalize_rect(x: f32, y: f32, w: f32, h: f32, width: u32, height: u32) -> (f32, f32, f32, f32) {
    if width == 0 || height == 0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let (fw, fh) = (width as f32, height as f32);
    let nx = (x / fw).clamp(0.0, 1.0);
    let ny = (y / fh).clamp(0.0, 1.0);
    let nw = (w / fw).clamp(0.0, 1.0 - nx);
    let nh = (h / fh).clamp(0.0, 1.0 - ny);
    (nx, ny, nw, nh)
}

/// Runs OCR on the STA worker thread from a ready **BGRA8** buffer (the swap already
/// happened on the blocking pool): wraps it in a `SoftwareBitmap`, awaits the
/// recognizer synchronously (safe on the STA thread), joins the lines into the raw
/// text, and walks `Lines().Words()` into per-word [`TextSpan`]s with normalized
/// `[0,1]` geometry (`03 §3/§3b`). WinRT exposes no per-word confidence, so every
/// row keeps the [`CONFIDENCE_UNKNOWN`] sentinel; spans are emitted as
/// [`TextRole::Unknown`] (PR3 classifies roles).
fn recognize_blocking(
    engine: &OcrEngine,
    bgra: &[u8],
    width: u32,
    height: u32,
) -> Result<OcrResult> {
    let buffer = CryptographicBuffer::CreateFromByteArray(bgra)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        width as i32,
        height as i32,
    )?;

    // `.join()` blocks until the async op completes (windows-future 0.3 renamed
    // the old `.get()`) — fine on the dedicated STA thread (we never .await inside
    // COM code).
    let result = engine.RecognizeAsync(&bitmap)?.join()?;

    let mut text = String::new();
    let mut spans: Vec<TextSpan> = Vec::new();
    // `Lines()` order is reading order; the index groups a line's words so PR3's
    // classifier reconstructs lines exactly (no geometry heuristic, `03 §3b`).
    for (line_index, line) in result.Lines()?.into_iter().enumerate() {
        text.push_str(&line.Text()?.to_string());
        text.push('\n');
        for word in line.Words()? {
            let word_text = word.Text()?.to_string();
            // WinRT `BoundingRect` is in pixels relative to the recognized bitmap.
            let rect = word.BoundingRect()?;
            let (x, y, w, h) =
                normalize_rect(rect.X, rect.Y, rect.Width, rect.Height, width, height);
            spans.push(TextSpan {
                normalized_text: normalize_text(&word_text),
                text: word_text,
                source: TextSource::Ocr,
                role: TextRole::Unknown,
                x,
                y,
                w,
                h,
                line_index: line_index as u32,
                is_searchable: true,
                suppress_reason: None,
            });
        }
    }

    Ok(OcrResult {
        text: text.trim_end().to_string(),
        mean_confidence: CONFIDENCE_UNKNOWN,
        engine: "winrt".to_string(),
        spans,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::sync::Arc;

    fn solid_frame(w: u32, h: u32) -> CapturedFrame {
        CapturedFrame {
            monitor_index: 0,
            width: w,
            height: h,
            captured_at: 1_000,
            pixels: Arc::new(RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))),
            content_hash: "test".to_string(),
            app_hint: None,
            window_title: None,
            target_rect: None,
        }
    }

    /// A typical box maps proportionally; a box that overruns the frame is clamped so
    /// it stays in `[0,1]` and `x + w <= 1`, `y + h <= 1`; a zero-area frame is a zero
    /// box. Pure helper — no WinRT runtime needed, so this runs in CI.
    #[test]
    fn normalize_rect_maps_and_clamps_to_unit_square() {
        // 100×200 box at (50, 40) inside a 200×400 frame → (0.25, 0.1, 0.5, 0.5).
        let (x, y, w, h) = normalize_rect(50.0, 40.0, 100.0, 200.0, 200, 400);
        assert!((x - 0.25).abs() < 1e-6, "x = {x}");
        assert!((y - 0.10).abs() < 1e-6, "y = {y}");
        assert!((w - 0.50).abs() < 1e-6, "w = {w}");
        assert!((h - 0.50).abs() < 1e-6, "h = {h}");

        // A box running past the right/bottom edge clamps so x+w<=1 and y+h<=1.
        let (x, y, w, h) = normalize_rect(180.0, 360.0, 120.0, 240.0, 200, 400);
        assert!(x + w <= 1.0 + 1e-6, "x+w = {}", x + w);
        assert!(y + h <= 1.0 + 1e-6, "y+h = {}", y + h);

        // Degenerate frame → zero box, never a divide-by-zero / NaN.
        assert_eq!(
            normalize_rect(10.0, 10.0, 5.0, 5.0, 0, 0),
            (0.0, 0.0, 0.0, 0.0)
        );
    }

    /// Real WinRT OCR on a blank white image: must return a result (empty text is
    /// fine) with the unknown-confidence sentinel and the `winrt` engine tag, and
    /// every emitted span's bounding box must be normalized to `[0,1]` with
    /// `x + w <= 1`, `y + h <= 1` (`03 §3b`). `#[ignore]`d in CI (needs the WinRT OCR
    /// runtime / language pack); run locally with `cargo test -p ocr -- --ignored`
    /// (`03 §10/§11`).
    #[tokio::test]
    #[ignore = "requires WinRT OCR language pack; run locally"]
    async fn winrt_ocr_recognizes_blank_image() {
        let ocr = WinRtOcr::spawn().expect("spawn OCR worker");
        let result = ocr.recognize(&solid_frame(64, 32)).await.expect("ocr ok");
        assert_eq!(result.engine, "winrt");
        assert_eq!(result.mean_confidence, CONFIDENCE_UNKNOWN);
        for span in &result.spans {
            assert!(
                (0.0..=1.0).contains(&span.x)
                    && (0.0..=1.0).contains(&span.y)
                    && span.x + span.w <= 1.0 + 1e-6
                    && span.y + span.h <= 1.0 + 1e-6,
                "span bbox not normalized: {span:?}"
            );
        }
    }
}
