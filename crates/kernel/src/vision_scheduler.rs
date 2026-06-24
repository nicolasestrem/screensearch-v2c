//! Timer + idle producers for deferred vision tagging (`03 §5`). Vision is **never**
//! real-time: `vision_tag` jobs are only enqueued on a user-configured timer or when
//! the OS reports the user idle past a threshold — both opt-in, off by default
//! (`06_PATCH_PLAN`, `07` gap #1). On-demand tagging goes through
//! [`crate::Kernel::enqueue_vision`] instead.
//!
//! Each enabled trigger enqueues up to `enrich.vision_batch_size` still-untagged frames
//! per tick. A frame that already has an in-flight (`pending`/`running`) `vision_tag`
//! job is skipped — [`Store::untagged_frame_ids`] excludes it — so a slow batch is not
//! re-enqueued on the next tick and the timer/idle lanes don't double-queue the same
//! frames. (`insert_vision` is still an idempotent upsert, so a stray re-analyze after a
//! job leaves the queue can't corrupt data.)

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use traits::{JobKind, NewJob, Store};

use crate::settings;
use crate::IdleSource;

/// How often the timer loop re-checks settings while the timer trigger is disabled.
const DISABLED_RECHECK: Duration = Duration::from_secs(30);
/// How often the idle loop samples the platform idle time.
const IDLE_POLL: Duration = Duration::from_secs(30);
/// Floor on the user-set timer interval, so a misconfigured tiny value can't busy-loop.
const MIN_TIMER_INTERVAL: Duration = Duration::from_secs(60);

/// Handle to the running scheduler tasks; stop them via [`SchedulerHandle::stop`].
pub(crate) struct SchedulerHandle {
    stop: watch::Sender<bool>,
    joins: Vec<JoinHandle<()>>,
}

impl SchedulerHandle {
    /// Signals the loops to stop and waits for them to finish.
    pub(crate) async fn stop(mut self) {
        let _ = self.stop.send(true);
        for join in std::mem::take(&mut self.joins) {
            let _ = join.await;
        }
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        // Safety net: if dropped without an explicit `stop`, still end the loops.
        let _ = self.stop.send(true);
    }
}

/// Spawns the timer loop (always) and, when an idle source is available, the idle loop.
/// Both honor their settings toggle each tick, so enabling/disabling in settings takes
/// effect without a restart.
pub(crate) fn spawn(store: Arc<dyn Store>, idle: Option<IdleSource>) -> SchedulerHandle {
    let (stop_tx, stop_rx) = watch::channel(false);
    let mut joins = vec![tokio::spawn(timer_loop(store.clone(), stop_rx.clone()))];
    if let Some(idle) = idle {
        joins.push(tokio::spawn(idle_loop(store, idle, stop_rx)));
    }
    SchedulerHandle {
        stop: stop_tx,
        joins,
    }
}

/// Enqueues up to `enrich.vision_batch_size` untagged frames every
/// `enrich.vision_timer_interval_ms`, while the timer trigger is enabled.
async fn timer_loop(store: Arc<dyn Store>, mut stop: watch::Receiver<bool>) {
    loop {
        let s = settings::load_settings(store.as_ref()).await;
        let wait = if s.enrich_vision_timer_enabled {
            Duration::from_millis(s.enrich_vision_timer_interval_ms as u64).max(MIN_TIMER_INTERVAL)
        } else {
            DISABLED_RECHECK
        };
        if wait_or_stop(&mut stop, wait).await {
            break;
        }
        // Re-read in case the toggle (or batch size) changed during the wait.
        let s = settings::load_settings(store.as_ref()).await;
        if s.enrich_vision_timer_enabled {
            enqueue_untagged(&store, "timer", s.enrich_vision_batch_size).await;
        }
    }
}

/// Enqueues a batch when the user transitions *into* idle (idle ≥ threshold), while the
/// idle trigger is enabled. Only on the transition, so a long idle period doesn't
/// re-enqueue every poll.
async fn idle_loop(store: Arc<dyn Store>, idle: IdleSource, mut stop: watch::Receiver<bool>) {
    let mut was_idle = false;
    loop {
        if wait_or_stop(&mut stop, IDLE_POLL).await {
            break;
        }
        let s = settings::load_settings(store.as_ref()).await;
        if !s.enrich_vision_idle_enabled {
            was_idle = false;
            continue;
        }
        let threshold_ms = (s.enrich_vision_idle_secs as u64).saturating_mul(1000);
        let now_idle = idle().is_some_and(|idle_ms| idle_ms >= threshold_ms);
        if now_idle && !was_idle {
            enqueue_untagged(&store, "idle", s.enrich_vision_batch_size).await;
        }
        was_idle = now_idle;
    }
}

/// Enqueues up to `batch` still-untagged frames (oldest first, excluding any with an
/// in-flight `vision_tag` job) as `vision_tag` jobs.
async fn enqueue_untagged(store: &Arc<dyn Store>, trigger: &str, batch: u32) {
    let ids = match store.untagged_frame_ids(batch, None).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(trigger, error = %e, "vision scheduler: list untagged failed");
            return;
        }
    };
    let mut count = 0u64;
    for frame_id in ids {
        let job = NewJob {
            kind: JobKind::VisionTag,
            frame_id: Some(frame_id),
            priority: 0,
            max_attempts: 3,
            not_before: 0,
        };
        match store.enqueue_job(job).await {
            Ok(_) => count += 1,
            Err(e) => tracing::warn!(frame_id, error = %e, "vision scheduler: enqueue failed"),
        }
    }
    if count > 0 {
        tracing::info!(trigger, count, "vision scheduler enqueued vision_tag jobs");
    }
}

/// Sleeps `dur`, returning early with `true` if a stop was signalled (so the caller
/// breaks its loop). Returns `true` if it should stop, `false` to continue.
async fn wait_or_stop(stop: &mut watch::Receiver<bool>, dur: Duration) -> bool {
    tokio::select! {
        biased;
        _ = stop.changed() => true,
        _ = tokio::time::sleep(dur) => *stop.borrow(),
    }
}
