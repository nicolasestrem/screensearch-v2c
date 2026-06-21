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
    };

    let mut cap = WgcCapture::new(config).expect("create WgcCapture");
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
