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
use traits::{CapturedFrame, OcrProvider, OcrResult, Result};

use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

/// Sentinel `mean_confidence` for OCR rows: WinRT `Media.Ocr` provides no
/// confidence, so we record "unknown" rather than inventing a number (`06`/`07`).
pub const CONFIDENCE_UNKNOWN: f32 = -1.0;

/// One OCR request handed to the STA worker. Carries owned RGBA bytes (Send) so no
/// WinRT object ever crosses the thread boundary.
struct Request {
    rgba: Vec<u8>,
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
        // Copy the pixels out (Send) so the WinRT work stays on the STA thread.
        let rgba = frame.pixels.as_raw().clone();
        let (width, height) = (frame.width, frame.height);
        let tx = self.tx.lock().expect("ocr sender poisoned").clone();

        tokio::task::spawn_blocking(move || -> Result<OcrResult> {
            let (resp_tx, resp_rx) = mpsc::channel();
            tx.send(Request {
                rgba,
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
        let result = recognize_blocking(&engine, &req.rgba, req.width, req.height);
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

/// Runs OCR on the worker thread. Converts RGBA → BGRA8 `SoftwareBitmap`, awaits
/// the recognizer synchronously (safe on the STA thread), and joins the lines.
fn recognize_blocking(
    engine: &OcrEngine,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<OcrResult> {
    // WinRT OCR wants a BGRA8 bitmap; the capture pipeline produces RGBA8.
    let mut bgra = rgba.to_vec();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }

    let buffer = CryptographicBuffer::CreateFromByteArray(&bgra)?;
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
    for line in result.Lines()? {
        text.push_str(&line.Text()?.to_string());
        text.push('\n');
    }

    Ok(OcrResult {
        text: text.trim_end().to_string(),
        mean_confidence: CONFIDENCE_UNKNOWN,
        engine: "winrt".to_string(),
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
        }
    }

    /// Real WinRT OCR on a blank white image: must return a result (empty text is
    /// fine) with the unknown-confidence sentinel and the `winrt` engine tag.
    /// `#[ignore]`d in CI (needs the WinRT OCR runtime / language pack); run locally
    /// with `cargo test -p ocr -- --ignored` (`03 §10/§11`).
    #[tokio::test]
    #[ignore = "requires WinRT OCR language pack; run locally"]
    async fn winrt_ocr_recognizes_blank_image() {
        let ocr = WinRtOcr::spawn().expect("spawn OCR worker");
        let result = ocr.recognize(&solid_frame(64, 32)).await.expect("ocr ok");
        assert_eq!(result.engine, "winrt");
        assert_eq!(result.mean_confidence, CONFIDENCE_UNKNOWN);
    }
}
