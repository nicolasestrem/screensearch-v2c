//! `ocr` — `OcrProvider` via WinRT `Media.Ocr` on a COM STA worker (`03 §3`).
//!
//! Native, no model download (~70–80 ms/frame, `01 §5`). WinRT OCR is COM
//! STA-bound: calls run on the thread that initialized COM (`01 §6`). OCR runs on
//! the **full-resolution** frame before any JPEG resize (`03 §8`).
//!
//! **Status:** P0 scaffold — no implementation yet. Lands in **P2** (`02 §5`).
//!
//! (No `forbid(unsafe_code)` here: the P2 WinRT/COM path will require `unsafe` FFI.)

/// The contract this crate implements (impl arrives in P2).
pub use traits::OcrProvider;
