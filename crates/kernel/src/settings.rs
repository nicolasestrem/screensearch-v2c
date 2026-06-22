//! Typed settings loading: assemble the strongly-typed [`Settings`] from the
//! opaque key/value `settings` table (`03 §8`).
//!
//! Each `03 §8` key is read individually; a missing or unparsable value falls back
//! to the corresponding [`Settings::default`] field (never an error — capture must
//! be able to start on a fresh DB). Composite values (`capture.monitors`,
//! `privacy.excluded_apps`, model tiers) are stored as JSON.

use serde::de::DeserializeOwned;
use std::str::FromStr;
use traits::{CaptureConfig, Result, Settings, Store};

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

/// Persists every `03 §8` setting back to the key/value `settings` table — the
/// exact inverse of [`load_settings`], using the **same key strings** so a saved
/// value round-trips. Numbers are written via `to_string`, bools as `"true"`/
/// `"false"`, and composite values (`capture.monitors`, `privacy.excluded_apps`,
/// model tiers) as JSON (matching how `set_model_tier` already writes a tier).
///
/// Backs the `set_settings` command (`03 §7`): the values are durable immediately;
/// which subsystems re-read them live vs on restart is documented in the Settings
/// UI (model tiers hot-apply; capture/storage/privacy on next capture start; the
/// rest on app restart).
pub async fn save_settings(store: &dyn Store, s: &Settings) -> Result<()> {
    store
        .set_setting("capture.interval_ms", &s.capture_interval_ms.to_string())
        .await?;
    store
        .set_setting(
            "capture.monitors",
            &serde_json::to_string(&s.capture_monitors)?,
        )
        .await?;
    store
        .set_setting(
            "capture.diff_threshold",
            &s.capture_diff_threshold.to_string(),
        )
        .await?;
    store
        .set_setting("storage.jpeg_quality", &s.storage_jpeg_quality.to_string())
        .await?;
    store
        .set_setting("storage.max_width", &s.storage_max_width.to_string())
        .await?;
    store
        .set_setting(
            "storage.retention_days",
            &s.storage_retention_days.to_string(),
        )
        .await?;
    store
        .set_setting("enrich.embed_text", bool_str(s.enrich_embed_text))
        .await?;
    store
        .set_setting(
            "enrich.image_embeddings",
            bool_str(s.enrich_image_embeddings),
        )
        .await?;
    store
        .set_setting(
            "enrich.vision_timer_enabled",
            bool_str(s.enrich_vision_timer_enabled),
        )
        .await?;
    store
        .set_setting(
            "enrich.vision_timer_interval_ms",
            &s.enrich_vision_timer_interval_ms.to_string(),
        )
        .await?;
    store
        .set_setting(
            "enrich.vision_idle_enabled",
            bool_str(s.enrich_vision_idle_enabled),
        )
        .await?;
    store
        .set_setting(
            "enrich.vision_idle_secs",
            &s.enrich_vision_idle_secs.to_string(),
        )
        .await?;
    store
        .set_setting(
            "enrich.worker_concurrency",
            &s.enrich_worker_concurrency.to_string(),
        )
        .await?;
    store
        .set_setting(
            "models.vision_tier",
            &serde_json::to_string(&s.models_vision_tier)?,
        )
        .await?;
    store
        .set_setting(
            "models.answer_tier",
            &serde_json::to_string(&s.models_answer_tier)?,
        )
        .await?;
    store
        .set_setting("answer.thinking", bool_str(s.answer_thinking))
        .await?;
    store
        .set_setting(
            "sidecar.idle_ttl_secs",
            &s.sidecar_idle_ttl_secs.to_string(),
        )
        .await?;
    store
        .set_setting("sidecar.ngl", &s.sidecar_ngl.to_string())
        .await?;
    store
        .set_setting(
            "privacy.excluded_apps",
            &serde_json::to_string(&s.privacy_excluded_apps)?,
        )
        .await?;
    store
        .set_setting("privacy.pause_on_lock", bool_str(s.privacy_pause_on_lock))
        .await?;
    Ok(())
}

/// The canonical string form for a persisted bool (parsed back by [`boolean`]).
fn bool_str(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
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
