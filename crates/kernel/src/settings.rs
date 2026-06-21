//! Typed settings loading: assemble the strongly-typed [`Settings`] from the
//! opaque key/value `settings` table (`03 §8`).
//!
//! Each `03 §8` key is read individually; a missing or unparsable value falls back
//! to the corresponding [`Settings::default`] field (never an error — capture must
//! be able to start on a fresh DB). Composite values (`capture.monitors`,
//! `privacy.excluded_apps`, model tiers) are stored as JSON.

use serde::de::DeserializeOwned;
use std::str::FromStr;
use traits::{CaptureConfig, Settings, Store};

/// Reads every `03 §8` setting, falling back to [`Settings::default`] per key.
pub async fn load_settings(store: &dyn Store) -> Settings {
    let d = Settings::default();
    Settings {
        capture_interval_ms: num(store, "capture.interval_ms", d.capture_interval_ms).await,
        capture_monitors: json(store, "capture.monitors", d.capture_monitors).await,
        capture_diff_threshold: num(store, "capture.diff_threshold", d.capture_diff_threshold)
            .await,
        storage_jpeg_quality: num(store, "storage.jpeg_quality", d.storage_jpeg_quality).await,
        storage_max_width: num(store, "storage.max_width", d.storage_max_width).await,
        storage_retention_days: num(store, "storage.retention_days", d.storage_retention_days)
            .await,
        enrich_embed_text: boolean(store, "enrich.embed_text", d.enrich_embed_text).await,
        enrich_image_embeddings: boolean(
            store,
            "enrich.image_embeddings",
            d.enrich_image_embeddings,
        )
        .await,
        enrich_vision_timer_enabled: boolean(
            store,
            "enrich.vision_timer_enabled",
            d.enrich_vision_timer_enabled,
        )
        .await,
        enrich_vision_timer_interval_ms: num(
            store,
            "enrich.vision_timer_interval_ms",
            d.enrich_vision_timer_interval_ms,
        )
        .await,
        enrich_vision_idle_enabled: boolean(
            store,
            "enrich.vision_idle_enabled",
            d.enrich_vision_idle_enabled,
        )
        .await,
        enrich_vision_idle_secs: num(store, "enrich.vision_idle_secs", d.enrich_vision_idle_secs)
            .await,
        enrich_worker_concurrency: num(
            store,
            "enrich.worker_concurrency",
            d.enrich_worker_concurrency,
        )
        .await,
        models_vision_tier: json(store, "models.vision_tier", d.models_vision_tier).await,
        models_answer_tier: json(store, "models.answer_tier", d.models_answer_tier).await,
        answer_thinking: boolean(store, "answer.thinking", d.answer_thinking).await,
        sidecar_idle_ttl_secs: num(store, "sidecar.idle_ttl_secs", d.sidecar_idle_ttl_secs).await,
        sidecar_ngl: num(store, "sidecar.ngl", d.sidecar_ngl).await,
        privacy_excluded_apps: json(store, "privacy.excluded_apps", d.privacy_excluded_apps).await,
        privacy_pause_on_lock: boolean(store, "privacy.pause_on_lock", d.privacy_pause_on_lock)
            .await,
    }
}

/// The capture-relevant slice of [`Settings`], handed to the capture impl (`03 §8`).
pub fn capture_config(s: &Settings) -> CaptureConfig {
    CaptureConfig {
        interval_ms: s.capture_interval_ms,
        monitors: s.capture_monitors.clone(),
        diff_threshold: s.capture_diff_threshold,
        excluded_apps: s.privacy_excluded_apps.clone(),
        pause_on_lock: s.privacy_pause_on_lock,
    }
}

async fn num<T: FromStr>(store: &dyn Store, key: &str, default: T) -> T {
    match store.get_setting(key).await {
        Ok(Some(raw)) => match raw.parse() {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(key, raw = %raw, "settings: unparsable number; using default");
                default
            }
        },
        Ok(None) => default,
        Err(e) => {
            tracing::warn!(key, error = %e, "settings: read failed; using default");
            default
        }
    }
}

async fn boolean(store: &dyn Store, key: &str, default: bool) -> bool {
    match store.get_setting(key).await {
        Ok(Some(raw)) => match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => {
                tracing::warn!(key, raw = %raw, "settings: unparsable bool; using default");
                default
            }
        },
        Ok(None) => default,
        Err(e) => {
            tracing::warn!(key, error = %e, "settings: read failed; using default");
            default
        }
    }
}

async fn json<T: DeserializeOwned>(store: &dyn Store, key: &str, default: T) -> T {
    match store.get_setting(key).await {
        Ok(Some(raw)) => match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(key, error = %e, "settings: unparsable JSON; using default");
                default
            }
        },
        Ok(None) => default,
        Err(e) => {
            tracing::warn!(key, error = %e, "settings: read failed; using default");
            default
        }
    }
}
