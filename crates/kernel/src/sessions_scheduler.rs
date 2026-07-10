//! Incremental session reconciliation + one-shot resumable historical backfill (`03 §7e`).
//! Every error is logged and swallowed by the scheduler loop: capture/OCR/storage never wait on
//! sessions, so the D10 degradation mode is always "no sessions", never lost frames.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use traits::{
    NewSessionArtifact, SegmentationParams, SegmenterFrame, Session, SessionArtifactKind,
    SessionDraft, SessionFilter, SessionKind, SessionSegmenter, Settings, Store,
};

use crate::{settings, worker_pool};

const CADENCE: Duration = Duration::from_secs(60);
const BACKFILL_CHUNK_MS: i64 = 6 * 60 * 60 * 1000;
const BACKFILL_MARKER: &str = "sessions.backfill_done_until";

pub(crate) struct SchedulerHandle {
    stop: watch::Sender<bool>,
    join: Option<JoinHandle<()>>,
}

impl SchedulerHandle {
    pub(crate) async fn stop(mut self) {
        let _ = self.stop.send(true);
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

pub(crate) fn spawn(
    store: Arc<dyn Store>,
    segmenter: Arc<dyn SessionSegmenter>,
    throttle_level: Arc<AtomicU8>,
) -> SchedulerHandle {
    let (stop_tx, stop_rx) = watch::channel(false);
    let join = tokio::spawn(scheduler_loop(store, segmenter, throttle_level, stop_rx));
    SchedulerHandle {
        stop: stop_tx,
        join: Some(join),
    }
}

async fn scheduler_loop(
    store: Arc<dyn Store>,
    segmenter: Arc<dyn SessionSegmenter>,
    throttle_level: Arc<AtomicU8>,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        let now = worker_pool::now_ms();
        let current = settings::load_settings(store.as_ref()).await;
        if let Err(error) =
            run_incremental_pass(store.as_ref(), segmenter.as_ref(), now, &current).await
        {
            tracing::warn!(%error, "sessions incremental pass failed; capture remains active");
        }
        let params = params_from_settings(now, &current);
        if let Err(error) = store
            .freeze_sessions(now.saturating_sub(params.freeze_lookback_ms))
            .await
        {
            tracing::warn!(%error, "sessions freeze pass failed; capture remains active");
        }
        if throttle_level.load(Ordering::Relaxed) == 0 {
            if let Err(error) =
                run_backfill_chunk(store.as_ref(), segmenter.as_ref(), now, &current).await
            {
                tracing::warn!(%error, "sessions historical pass failed; capture remains active");
            }
        } else {
            tracing::debug!("sessions historical pass yielded to enrichment throttle");
        }
        if wait_or_stop(&mut stop, CADENCE).await {
            break;
        }
    }
}

fn params_from_settings(now_ms: i64, settings: &Settings) -> SegmentationParams {
    SegmentationParams::shipped(
        now_ms,
        i64::from(settings.sessions_gap_close_secs).saturating_mul(1000),
        i64::from(settings.sessions_min_len_secs).saturating_mul(1000),
    )
}

fn partition(kind: SessionKind, context_key: &str) -> String {
    if kind == SessionKind::Focus {
        "focus".to_string()
    } else {
        context_key.to_string()
    }
}

/// Drops/trims drafts that reach into an already-frozen span of the same identity track.
/// Cross-track overlap remains untouched. The incremental window meets frozen history at its
/// old edge, so the safe retained portion is the tail strictly after the latest overlapping frozen
/// end (session `ended_at` is the timestamp of its last owned frame, not an exclusive bound).
pub(crate) fn trim_drafts_against_frozen(
    drafts: Vec<SessionDraft>,
    frozen: &[Session],
    frames: &[SegmenterFrame],
) -> Vec<SessionDraft> {
    let frame_times: HashMap<i64, i64> = frames
        .iter()
        .map(|frame| (frame.id, frame.captured_at))
        .collect();
    drafts
        .into_iter()
        .filter_map(|mut draft| {
            let own_partition = partition(draft.kind, &draft.context_key);
            let cut = frozen
                .iter()
                .filter(|session| session.frozen)
                .filter(|session| partition(session.kind, &session.context_key) == own_partition)
                .filter_map(|session| {
                    let end = session.ended_at?;
                    (draft.started_at < end && draft.ended_at > session.started_at).then_some(end)
                })
                .max();
            if let Some(cut) = cut {
                draft.frame_ids.retain(|id| {
                    frame_times
                        .get(id)
                        .is_some_and(|captured_at| *captured_at > cut)
                });
                if draft.frame_ids.is_empty() {
                    return None;
                }
                let times: Vec<i64> = draft
                    .frame_ids
                    .iter()
                    .filter_map(|id| frame_times.get(id).copied())
                    .collect();
                draft.started_at = *times.iter().min()?;
                draft.ended_at = *times.iter().max()?;
            }
            Some(draft)
        })
        .collect()
}

fn overlap_ms(draft: &SessionDraft, session: &Session, now_ms: i64) -> Option<i64> {
    let session_end = session.ended_at.unwrap_or(now_ms);
    let overlap = draft.ended_at.min(session_end) - draft.started_at.max(session.started_at);
    (overlap >= 0).then_some(overlap)
}

/// One-to-one, deterministic draft→row matching by exact context key + maximum time overlap.
pub(crate) fn match_drafts_to_existing(
    drafts: &[SessionDraft],
    existing: &[Session],
    now_ms: i64,
) -> Vec<Option<i64>> {
    let mut used = HashSet::new();
    drafts
        .iter()
        .map(|draft| {
            let best = existing
                .iter()
                .filter(|session| !session.frozen)
                .filter(|session| session.context_key == draft.context_key)
                .filter(|session| !used.contains(&session.id))
                .filter_map(|session| {
                    overlap_ms(draft, session, now_ms).map(|overlap| (session, overlap))
                })
                .max_by(|(left, left_overlap), (right, right_overlap)| {
                    left_overlap
                        .cmp(right_overlap)
                        .then_with(|| {
                            let left_delta = (draft.started_at - left.started_at).abs();
                            let right_delta = (draft.started_at - right.started_at).abs();
                            right_delta.cmp(&left_delta)
                        })
                        .then_with(|| right.id.cmp(&left.id))
                })
                .map(|(session, _)| session.id);
            if let Some(id) = best {
                used.insert(id);
            }
            best
        })
        .collect()
}

pub(crate) async fn run_incremental_pass(
    store: &dyn Store,
    segmenter: &dyn SessionSegmenter,
    now_ms: i64,
    current: &Settings,
) -> anyhow::Result<()> {
    let params = params_from_settings(now_ms, current);
    let existing = store.unfrozen_sessions().await?;
    let mutable_tail = now_ms.saturating_sub(params.freeze_lookback_ms);
    let from_ms = existing
        .iter()
        .map(|session| session.started_at)
        .min()
        .map_or(mutable_tail, |earliest| earliest.min(mutable_tail));
    let frames = store
        .frames_meta_in_range(from_ms, now_ms.saturating_add(1))
        .await?;
    let drafts = segmenter.segment(&frames, &params);
    let frozen = store
        .sessions_in_range(SessionFilter {
            from_ms: Some(from_ms),
            to_ms: Some(now_ms.saturating_add(1)),
            now_ms,
            ..SessionFilter::default()
        })
        .await?
        .into_iter()
        .filter(|session| session.frozen)
        .collect::<Vec<_>>();
    let drafts = trim_drafts_against_frozen(drafts, &frozen, &frames);
    let matches = match_drafts_to_existing(&drafts, &existing, now_ms);

    // Clear only mutable ownership before rebuilding it. The store's SQL guard makes this a
    // no-op for any frame whose current session froze between the read and this write.
    for session in &existing {
        let ids: Vec<i64> = store
            .session_frames_meta(session.id)
            .await?
            .into_iter()
            .map(|frame| frame.id)
            .collect();
        store.assign_frames_session(&ids, None).await?;
    }

    let matched_ids: HashSet<i64> = matches.iter().flatten().copied().collect();
    for session in &existing {
        if !matched_ids.contains(&session.id) {
            store.delete_unfrozen_session(session.id).await?;
        }
    }

    for (draft, matched) in drafts.iter().zip(matches) {
        let session_id = match matched {
            Some(id) => {
                if !store
                    .update_unfrozen_session(id, draft.as_new_session(false))
                    .await?
                {
                    continue;
                }
                id
            }
            None => store.insert_session(draft.as_new_session(false)).await?,
        };
        store
            .assign_frames_session(&draft.frame_ids, Some(session_id))
            .await?;
        if draft.kind == SessionKind::Ai {
            refresh_exchanges(store, segmenter, session_id, draft).await?;
        }
    }
    Ok(())
}

async fn refresh_exchanges(
    store: &dyn Store,
    segmenter: &dyn SessionSegmenter,
    session_id: i64,
    draft: &SessionDraft,
) -> anyhow::Result<()> {
    let contents = store.content_texts_for_frames(&draft.frame_ids).await?;
    let extracted = segmenter.extract_exchanges(draft.tool.as_deref().unwrap_or(""), &contents);
    store
        .delete_session_artifacts_by_kind(session_id, SessionArtifactKind::Exchange)
        .await?;
    let artifacts: Vec<NewSessionArtifact> = extracted
        .into_iter()
        .map(|exchange| NewSessionArtifact {
            kind: SessionArtifactKind::Exchange,
            role: Some(exchange.role),
            frame_id: Some(exchange.frame_id),
            content: exchange.content,
        })
        .collect();
    store
        .insert_session_artifacts(session_id, &artifacts)
        .await?;
    Ok(())
}

/// Returns the timestamp at which a chunk can safely stop: the first frame after a global
/// inactivity gap >= merge_gap, at/after the desired six-hour boundary. If the fixed target
/// itself is safely beyond the final frame, the desired boundary is the cut. `scan_limit_ms` may
/// advance beyond the checkpoint's fixed historical target on later ticks so a closing gap after a
/// target-cut continuous track can eventually be observed.
pub(crate) fn safe_backfill_cut(
    frames: &[SegmenterFrame],
    desired_end_ms: i64,
    merge_gap_ms: i64,
    scan_limit_ms: i64,
) -> Option<i64> {
    if frames.is_empty() {
        // Only the range through `desired_end_ms` is known empty. Advancing all the way to the
        // final target would silently skip frames that have not been scanned yet.
        return Some(desired_end_ms.min(scan_limit_ms));
    }
    for pair in frames.windows(2) {
        if pair[1].captured_at >= desired_end_ms
            && pair[1].captured_at - pair[0].captured_at >= merge_gap_ms
        {
            return Some(pair[1].captured_at.min(scan_limit_ms));
        }
    }
    let last = frames.last()?.captured_at;
    (desired_end_ms.saturating_sub(last) >= merge_gap_ms)
        .then_some(desired_end_ms.min(scan_limit_ms))
}

/// Historical backfill approaches frozen incremental history from the left. When a delayed retry
/// meets an already-frozen row on the same identity track, retain only the prefix strictly before
/// that row. This preserves frozen immutability, avoids duplicate overlapping rows, and keeps every
/// retained draft id assignable.
fn trim_backfill_draft_against_frozen(
    mut draft: SessionDraft,
    frozen: &[Session],
    frame_times: &HashMap<i64, i64>,
) -> Option<SessionDraft> {
    let own_partition = partition(draft.kind, &draft.context_key);
    let cut = frozen
        .iter()
        .filter(|session| session.frozen)
        .filter(|session| partition(session.kind, &session.context_key) == own_partition)
        .filter(|session| {
            let end = session.ended_at.unwrap_or(session.started_at);
            draft.started_at <= end && draft.ended_at >= session.started_at
        })
        .map(|session| session.started_at)
        .min()?;
    draft.frame_ids.retain(|id| {
        frame_times
            .get(id)
            .is_some_and(|captured_at| *captured_at < cut)
    });
    if draft.frame_ids.is_empty() {
        return None;
    }
    let times: Vec<i64> = draft
        .frame_ids
        .iter()
        .filter_map(|id| frame_times.get(id).copied())
        .collect();
    draft.started_at = *times.iter().min()?;
    draft.ended_at = *times.iter().max()?;
    Some(draft)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct BackfillCheckpoint {
    cursor_ms: i64,
    target_ms: i64,
}

async fn load_or_initialize_checkpoint(
    store: &dyn Store,
    target_ms: i64,
) -> anyhow::Result<BackfillCheckpoint> {
    if let Some(value) = store.get_setting(BACKFILL_MARKER).await? {
        return serde_json::from_str(&value).map_err(Into::into);
    }
    let earliest = store
        .sample_frames_in_range(i64::MIN, target_ms, 1)
        .await?
        .first()
        .map(|frame| frame.captured_at)
        .unwrap_or(target_ms);
    let checkpoint = BackfillCheckpoint {
        cursor_ms: earliest,
        target_ms,
    };
    save_checkpoint(store, checkpoint).await?;
    Ok(checkpoint)
}

async fn save_checkpoint(store: &dyn Store, checkpoint: BackfillCheckpoint) -> anyhow::Result<()> {
    store
        .set_setting(BACKFILL_MARKER, &serde_json::to_string(&checkpoint)?)
        .await
}

pub(crate) async fn run_backfill_chunk(
    store: &dyn Store,
    segmenter: &dyn SessionSegmenter,
    now_ms: i64,
    current: &Settings,
) -> anyhow::Result<()> {
    let params = params_from_settings(now_ms, current);
    let current_frozen_horizon = now_ms.saturating_sub(params.freeze_lookback_ms);
    let mut checkpoint = load_or_initialize_checkpoint(store, current_frozen_horizon).await?;
    if checkpoint.cursor_ms >= checkpoint.target_ms
        || current_frozen_horizon <= checkpoint.cursor_ms
    {
        return Ok(());
    }

    let desired = checkpoint
        .cursor_ms
        .saturating_add(BACKFILL_CHUNK_MS)
        .min(checkpoint.target_ms);
    let mut scan_end = desired
        .saturating_add(params.merge_gap_ms)
        .min(current_frozen_horizon);
    let frames = loop {
        let frames = store
            .frames_meta_in_range(checkpoint.cursor_ms, scan_end)
            .await?;
        if safe_backfill_cut(
            &frames,
            desired,
            params.merge_gap_ms,
            current_frozen_horizon,
        )
        .is_some()
            || scan_end >= current_frozen_horizon
        {
            break frames;
        }
        scan_end = scan_end
            .saturating_add(BACKFILL_CHUNK_MS)
            .min(current_frozen_horizon);
    };
    let Some(cut) = safe_backfill_cut(
        &frames,
        desired,
        params.merge_gap_ms,
        current_frozen_horizon,
    ) else {
        // The fixed historical target cuts through a still-continuous track. Leave the cursor
        // untouched; the incremental tail owns current work and a later pass can close safely.
        return Ok(());
    };
    let chunk: Vec<SegmenterFrame> = frames
        .into_iter()
        .filter(|frame| frame.captured_at < cut)
        .collect();
    if !chunk.is_empty() {
        let frame_times: HashMap<i64, i64> = chunk
            .iter()
            .map(|frame| (frame.id, frame.captured_at))
            .collect();
        let closed_params = SegmentationParams {
            now_ms: cut.saturating_add(params.merge_gap_ms),
            ..params
        };
        let drafts = segmenter.segment(&chunk, &closed_params);
        let existing = store
            .sessions_in_range(SessionFilter {
                from_ms: Some(checkpoint.cursor_ms),
                to_ms: Some(cut),
                now_ms,
                ..SessionFilter::default()
            })
            .await?;
        for draft in drafts {
            let existing_id = existing
                .iter()
                .find(|session| {
                    session.frozen
                        && session.context_key == draft.context_key
                        && session.started_at == draft.started_at
                        && session.ended_at == Some(draft.ended_at)
                })
                .map(|session| session.id);
            let (session_id, draft, inserted_unfrozen) = match existing_id {
                Some(id) => (id, draft, false),
                None => {
                    let overlapping_frozen = existing.iter().any(|session| {
                        session.frozen
                            && partition(session.kind, &session.context_key)
                                == partition(draft.kind, &draft.context_key)
                            && session.ended_at.is_some_and(|end| {
                                draft.started_at <= end && draft.ended_at >= session.started_at
                            })
                    });
                    let draft = if overlapping_frozen {
                        trim_backfill_draft_against_frozen(draft, &existing, &frame_times)
                    } else {
                        Some(draft)
                    };
                    let Some(draft) = draft else {
                        continue;
                    };
                    // Assign before freezing so a partial assignment can be rolled back without
                    // violating the immutable-frozen-row contract.
                    let id = store.insert_session(draft.as_new_session(false)).await?;
                    (id, draft, true)
                }
            };
            let already_owned: HashSet<i64> = store
                .session_frames_meta(session_id)
                .await?
                .into_iter()
                .map(|frame| frame.id)
                .collect();
            let expected_changes = draft
                .frame_ids
                .iter()
                .filter(|id| !already_owned.contains(id))
                .count() as u64;
            let changed = store
                .assign_frames_session(&draft.frame_ids, Some(session_id))
                .await?;
            if changed != expected_changes {
                if inserted_unfrozen {
                    store.delete_unfrozen_session(session_id).await?;
                }
                anyhow::bail!(
                    "historical session {session_id} assigned {changed}/{expected_changes} new frames"
                );
            }
            if inserted_unfrozen {
                store.freeze_sessions(current_frozen_horizon).await?;
                if !store
                    .get_session(session_id)
                    .await?
                    .is_some_and(|session| session.frozen)
                {
                    store.delete_unfrozen_session(session_id).await?;
                    anyhow::bail!(
                        "historical session {session_id} did not freeze after assignment"
                    );
                }
            }
            if draft.kind == SessionKind::Ai {
                refresh_exchanges(store, segmenter, session_id, &draft).await?;
            }
        }
    }
    checkpoint.cursor_ms = cut;
    save_checkpoint(store, checkpoint).await?;
    tracing::info!(
        cursor_ms = checkpoint.cursor_ms,
        target_ms = checkpoint.target_ms,
        "sessions historical backfill advanced"
    );
    Ok(())
}

async fn wait_or_stop(stop: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    tokio::select! {
        biased;
        _ = stop.changed() => true,
        _ = tokio::time::sleep(duration) => *stop.borrow(),
    }
}
