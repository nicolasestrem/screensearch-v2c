//! Windows-gated WGC smoke test (`03 §10/§11`). Captures one real frame from the
//! primary monitor. `#[ignore]`d in CI (needs a real desktop + GPU); run locally
//! with `cargo test -p capture -- --ignored`.

use std::time::Duration;

use capture::WgcCapture;
use traits::{CaptureConfig, CaptureSource};

#[tokio::test]
#[ignore = "requires a real desktop + GPU (WGC); run locally"]
async fn wgc_captures_a_frame_from_the_primary_monitor() {
    let config = CaptureConfig {
        interval_ms: 50,
        monitors: Vec::new(),
        diff_threshold: 0.0, // first frame always passes anyway
        excluded_apps: Vec::new(),
        pause_on_lock: false,
        event_driven_enabled: false,
        event_on_foreground: true,
        event_on_idle: false,
        event_debounce_ms: 500,
        event_min_interval_ms: 1000,
        event_idle_threshold_ms: 5000,
        event_fallback_interval_ms: 30_000,
    };

    // No marks in this smoke test — drop the demand sender so the channel stays closed.
    let (_capture_now_tx, capture_now_rx) = tokio::sync::mpsc::channel(1);
    let mut cap = WgcCapture::new(config, capture_now_rx).expect("create WgcCapture");
    assert!(!cap.monitors().is_empty(), "at least one monitor");

    let frame = tokio::time::timeout(Duration::from_secs(10), cap.next_frame())
        .await
        .expect("next_frame within 10s")
        .expect("next_frame ok")
        .expect("a changed frame");

    assert!(frame.width > 0 && frame.height > 0, "non-empty frame");
    assert_eq!(frame.pixels.width(), frame.width);
    assert_eq!(frame.pixels.height(), frame.height);
    assert_eq!(
        frame.pixels.as_raw().len(),
        (frame.width * frame.height * 4) as usize
    );
}
