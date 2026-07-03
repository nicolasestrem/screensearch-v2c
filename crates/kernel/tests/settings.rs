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
        models_answer_tier: ModelTier::Quality,
        answer_thinking: false,
        sidecar_idle_ttl_secs: 600,
        sidecar_ngl: 35,
        sidecar_device: Some("Vulkan0".to_string()),
        sidecar_ctx_size: 4096,
        sidecar_kv_cache_type: KvCacheType::F16,
        sidecar_flash_attn: FlashAttnSetting::Off,
        sidecar_recycle_enabled: false,
        sidecar_recycle_rss_mb: 8192,
        privacy_excluded_apps: vec!["Signal".to_string(), "Element".to_string()],
        privacy_pause_on_lock: false,
        text_include_chrome_default: true,
        text_chrome_suppress_min_seen: 20,
        text_chrome_protect_min_chars: 64,
        text_chrome_region_buckets: 12,
        retrieval_default_top_k: 16,
        reports_daily_top_k: 60,
        reports_weekly_top_k: 300,
        reports_map_reduce_min_frames: 30,
        // Event-driven capture — every field away from its default, within the
        // sanitize clamps, so the round-trip exercises the new load/save encodings.
        capture_event_driven_enabled: true,
        capture_event_on_foreground: false,
        capture_event_on_idle: true,
        capture_event_debounce_ms: 750,
        capture_event_min_interval_ms: 2000,
        capture_event_idle_threshold_ms: 8000,
        capture_event_fallback_interval_ms: 60_000,
        // UIA text — every field away from its default, within the sanitize clamps.
        capture_uia_text_enabled: false,
        capture_uia_latency_budget_ms: 300,
        capture_uia_min_text_chars: 32,
        // UIA hang-fix knobs (`07` #71) — each away from its default, within the clamps.
        capture_uia_run_on_interactive: true,
        capture_uia_view_control_only: false,
        capture_uia_max_nodes: 2000,
        capture_uia_max_textpattern_calls: 128,
        capture_uia_suppress_during_input_ms: 750,
        // Enrichment throttle — every field away from its default, within the sanitize
        // clamps (each exit % kept below its enter %), so the round-trip exercises the
        // new load/save encodings.
        throttle_enabled: true,
        throttle_cpu_enter_pct: 75.0,
        throttle_cpu_exit_pct: 50.0,
        throttle_gpu_enter_pct: 80.0,
        throttle_gpu_exit_pct: 55.0,
        throttle_enter_after_ms: 3000,
        throttle_exit_after_ms: 6000,
        throttle_sample_interval_ms: 2000,
        throttle_embed_text_floor: 2,
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
    store
        .set_setting("capture.uia_max_nodes", "50") // below floor 100
        .await
        .unwrap();
    store
        .set_setting("capture.uia_max_textpattern_calls", "0") // below floor 1
        .await
        .unwrap();
    store
        .set_setting("capture.uia_suppress_during_input_ms", "99999") // above ceiling 10_000
        .await
        .unwrap();

    let loaded = load_settings(dyn_store).await;

    assert_eq!(loaded.capture_interval_ms, 250);
    assert_eq!(loaded.capture_uia_max_nodes, 100);
    assert_eq!(loaded.capture_uia_max_textpattern_calls, 1);
    assert_eq!(loaded.capture_uia_suppress_during_input_ms, 10_000);
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
        capture_event_debounce_ms: 1,           // below floor 100
        capture_event_min_interval_ms: 999_999, // above ceiling 60_000
        capture_event_fallback_interval_ms: 1,  // below floor 1_000
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
    assert_eq!(loaded.capture_event_debounce_ms, 100);
    assert_eq!(loaded.capture_event_min_interval_ms, 60_000);
    assert_eq!(loaded.capture_event_fallback_interval_ms, 1_000);

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

#[tokio::test]
async fn load_drops_retired_event_keys_without_error() {
    // 0.3.0 PR2: a config persisted by an older version still carries the four extra
    // event-trigger keys + the typing-pause threshold. `drop_retired_settings` purges
    // them (so the row doesn't linger) and load must not error or be perturbed by them.
    use kernel::settings::{drop_retired_settings, RETIRED_SETTINGS_KEYS};

    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;

    // A real (surviving) key alongside the retired ones, to prove only the retired go.
    save_settings(dyn_store, &Settings::default())
        .await
        .expect("seed defaults");
    for key in RETIRED_SETTINGS_KEYS {
        store.set_setting(key, "true").await.unwrap();
    }

    drop_retired_settings(dyn_store).await;

    for key in RETIRED_SETTINGS_KEYS {
        assert_eq!(
            store.get_setting(key).await.unwrap(),
            None,
            "retired key {key} must be dropped"
        );
    }
    // A surviving key is untouched, and load still yields the defaults (no error).
    assert!(store
        .get_setting("capture.event_on_foreground")
        .await
        .unwrap()
        .is_some());
    assert_eq!(load_settings(dyn_store).await, Settings::default());

    // Idempotent: a second run finds nothing to drop (so it logs nothing — "once").
    drop_retired_settings(dyn_store).await;
}

#[tokio::test]
async fn persisted_beta_tier_remaps_to_quality_and_persists() {
    // 0.3.0 PR3 (D3): the Beta tier is retired. A config persisted by an older version
    // still carries `"beta"` for either lane; load must map it to Quality (never fall
    // back to Default) and persist the mapping — so the retired token leaves the DB and
    // a later load is clean ("logged once", the `drop_retired_settings` mechanism).
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;

    store
        .set_setting("models.vision_tier", "\"beta\"")
        .await
        .unwrap();
    store
        .set_setting("models.answer_tier", "\"beta\"")
        .await
        .unwrap();

    let loaded = load_settings(dyn_store).await;
    assert_eq!(loaded.models_vision_tier, ModelTier::Quality);
    assert_eq!(loaded.models_answer_tier, ModelTier::Quality);

    // The mapping is persisted: the retired token is gone, replaced by canonical JSON.
    assert_eq!(
        store
            .get_setting("models.vision_tier")
            .await
            .unwrap()
            .as_deref(),
        Some("\"quality\"")
    );
    assert_eq!(
        store
            .get_setting("models.answer_tier")
            .await
            .unwrap()
            .as_deref(),
        Some("\"quality\"")
    );

    // Idempotent: a second load reads `"quality"` and neither remaps nor rewrites.
    let reloaded = load_settings(dyn_store).await;
    assert_eq!(reloaded.models_vision_tier, ModelTier::Quality);
    assert_eq!(reloaded.models_answer_tier, ModelTier::Quality);
}

#[tokio::test]
async fn unknown_tier_falls_back_to_default_without_rewrite() {
    // A genuinely unparsable tier value is not the retired Beta token: it falls back to
    // the field default (Default), like any unparsable JSON, and — unlike Beta — is NOT
    // rewritten. Only the known-legacy `"beta"` value is migrated.
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;

    store
        .set_setting("models.vision_tier", "\"turbo\"")
        .await
        .unwrap();

    let loaded = load_settings(dyn_store).await;
    assert_eq!(loaded.models_vision_tier, ModelTier::Default);
    // The unknown value is left as-is (not migrated).
    assert_eq!(
        store
            .get_setting("models.vision_tier")
            .await
            .unwrap()
            .as_deref(),
        Some("\"turbo\"")
    );
}

#[tokio::test]
async fn save_settings_never_writes_retired_keys() {
    // The save path must not resurrect a retired key: after saving defaults, none of the
    // retired keys exist. This pins `save_settings`' key set against the retired list.
    use kernel::settings::RETIRED_SETTINGS_KEYS;

    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    let dyn_store: &dyn Store = &store;

    save_settings(dyn_store, &Settings::default())
        .await
        .expect("save settings");

    for key in RETIRED_SETTINGS_KEYS {
        assert_eq!(
            store.get_setting(key).await.unwrap(),
            None,
            "save_settings must not write retired key {key}"
        );
    }
}
