//! Settings persistence round-trip (`03 §8`): `save_settings` followed by
//! `load_settings` returns exactly what was written. This guards the key-string
//! contract between the two — a typo in either would silently fall back to a
//! default and never round-trip.

use kernel::settings::{load_settings, save_settings};
use store::SqliteStore;
use traits::{FlashAttnSetting, KvCacheType, ModelTier, Settings, Store};

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
        enrich_vision_batch_size: 50,
        enrich_worker_concurrency: 4,
        models_vision_tier: ModelTier::Quality,
        models_answer_tier: ModelTier::Beta,
        answer_thinking: false,
        sidecar_idle_ttl_secs: 600,
        sidecar_ngl: 35,
        sidecar_device: Some("Vulkan0".to_string()),
        sidecar_ctx_size: 4096,
        sidecar_kv_cache_type: KvCacheType::F16,
        sidecar_flash_attn: FlashAttnSetting::Off,
        privacy_excluded_apps: vec!["Signal".to_string(), "Element".to_string()],
        privacy_pause_on_lock: false,
    };

    save_settings(dyn_store, &original)
        .await
        .expect("save settings");
    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded, original, "non-default values must round-trip");
}

#[tokio::test]
async fn load_settings_sanitizes_persisted_numeric_values() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;

    store.set_setting("capture.interval_ms", "1").await.unwrap();
    store
        .set_setting("capture.diff_threshold", "NaN")
        .await
        .unwrap();
    store
        .set_setting("storage.jpeg_quality", "0")
        .await
        .unwrap();
    store
        .set_setting("storage.max_width", "100000")
        .await
        .unwrap();
    store
        .set_setting("enrich.worker_concurrency", "0")
        .await
        .unwrap();
    store
        .set_setting("enrich.vision_timer_interval_ms", "1")
        .await
        .unwrap();
    store
        .set_setting("enrich.vision_idle_secs", "1")
        .await
        .unwrap();
    store
        .set_setting("enrich.vision_batch_size", "9999")
        .await
        .unwrap();
    store
        .set_setting("sidecar.idle_ttl_secs", "999999")
        .await
        .unwrap();
    store.set_setting("sidecar.ngl", "10000").await.unwrap();
    store
        .set_setting("sidecar.ctx_size", "999999")
        .await
        .unwrap();

    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded.capture_interval_ms, 250);
    assert_eq!(loaded.capture_diff_threshold, 0.0);
    assert_eq!(loaded.storage_jpeg_quality, 1);
    assert_eq!(loaded.storage_max_width, 7680);
    assert_eq!(loaded.enrich_worker_concurrency, 1);
    assert_eq!(loaded.enrich_vision_timer_interval_ms, 60_000);
    assert_eq!(loaded.enrich_vision_idle_secs, 60);
    assert_eq!(loaded.enrich_vision_batch_size, 500);
    assert_eq!(loaded.sidecar_idle_ttl_secs, 86_400);
    assert_eq!(loaded.sidecar_ngl, 999);
    assert_eq!(loaded.sidecar_ctx_size, 32_768);
}

#[tokio::test]
async fn sidecar_ctx_size_zero_is_preserved_as_auto_sentinel() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;

    // 0 must survive sanitization (it means "automatic per-lane default"), not get
    // clamped up to the 512 floor.
    store.set_setting("sidecar.ctx_size", "0").await.unwrap();
    assert_eq!(load_settings(dyn_store).await.sidecar_ctx_size, 0);

    // A small non-zero value below the floor is clamped up.
    store.set_setting("sidecar.ctx_size", "100").await.unwrap();
    assert_eq!(load_settings(dyn_store).await.sidecar_ctx_size, 512);
}

#[tokio::test]
async fn save_settings_persists_sanitized_numeric_values() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;
    let original = Settings {
        capture_interval_ms: 1,
        capture_diff_threshold: f32::NAN,
        storage_jpeg_quality: 0,
        storage_max_width: 100_000,
        enrich_worker_concurrency: 0,
        enrich_vision_timer_interval_ms: 1,
        enrich_vision_idle_secs: 1,
        enrich_vision_batch_size: 9_999,
        sidecar_idle_ttl_secs: 999_999,
        sidecar_ngl: 10_000,
        sidecar_ctx_size: 999_999,
        ..Settings::default()
    };

    save_settings(dyn_store, &original)
        .await
        .expect("save settings");
    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded.capture_interval_ms, 250);
    assert_eq!(loaded.capture_diff_threshold, 0.0);
    assert_eq!(loaded.storage_jpeg_quality, 1);
    assert_eq!(loaded.storage_max_width, 7680);
    assert_eq!(loaded.enrich_worker_concurrency, 1);
    assert_eq!(loaded.enrich_vision_timer_interval_ms, 60_000);
    assert_eq!(loaded.enrich_vision_idle_secs, 60);
    assert_eq!(loaded.enrich_vision_batch_size, 500);
    assert_eq!(loaded.sidecar_idle_ttl_secs, 86_400);
    assert_eq!(loaded.sidecar_ngl, 999);
    assert_eq!(loaded.sidecar_ctx_size, 32_768);

    assert_eq!(
        store
            .get_setting("capture.diff_threshold")
            .await
            .unwrap()
            .as_deref(),
        Some("0")
    );
}

#[tokio::test]
async fn sidecar_device_round_trips_empty_as_none() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;
    let settings = Settings {
        sidecar_device: Some("   ".to_string()),
        ..Settings::default()
    };

    save_settings(dyn_store, &settings)
        .await
        .expect("save settings");
    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded.sidecar_device, None);
    assert_eq!(
        store
            .get_setting("sidecar.device")
            .await
            .unwrap()
            .as_deref(),
        Some("null")
    );
}
