//! Settings persistence round-trip (`03 §8`): `save_settings` followed by
//! `load_settings` returns exactly what was written. This guards the key-string
//! contract between the two — a typo in either would silently fall back to a
//! default and never round-trip.

use kernel::settings::{load_settings, save_settings};
use store::SqliteStore;
use traits::{ModelTier, Settings, Store};

#[tokio::test]
async fn round_trips_defaults() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;
    let original = Settings::default();

    save_settings(dyn_store, &original)
        .await
        .expect("save settings");
    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded, original, "defaults must round-trip");
}

#[tokio::test]
async fn round_trips_non_default_values() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;
    // Every field set away from its default, including composites (monitors,
    // excluded apps, model tiers) so the JSON encodings are exercised too.
    let original = Settings {
        capture_interval_ms: 5000,
        capture_monitors: vec![0, 2],
        capture_diff_threshold: 0.02,
        storage_jpeg_quality: 90,
        storage_max_width: 1600,
        storage_retention_days: 30,
        enrich_embed_text: false,
        enrich_image_embeddings: true,
        enrich_vision_timer_enabled: true,
        enrich_vision_timer_interval_ms: 1_800_000,
        enrich_vision_idle_enabled: true,
        enrich_vision_idle_secs: 120,
        enrich_worker_concurrency: 4,
        models_vision_tier: ModelTier::Quality,
        models_answer_tier: ModelTier::Beta,
        answer_thinking: false,
        sidecar_idle_ttl_secs: 600,
        sidecar_ngl: 35,
        privacy_excluded_apps: vec!["Signal".to_string(), "Element".to_string()],
        privacy_pause_on_lock: false,
    };

    save_settings(dyn_store, &original)
        .await
        .expect("save settings");
    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded, original, "non-default values must round-trip");
}
