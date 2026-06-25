//! Recall report generation — Calendar-Grid Coverage Map-Reduce (`03 §8b`,
//! `docs/0.2.0.md` PR6).
//!
//! A weekly report must cover the *whole* week, not just its most-recent or
//! most-relevant frames. So the orchestrator does **not** grow the model context
//! window (that scales KV-cache VRAM and forces a sidecar relaunch); instead the
//! model window stays pinned flat and the **number of bounded passes scales with the
//! range**: the range is split into a per-period grid (one calendar period each),
//! every active period gets its own MAP pass with its own frame budget — so a dense
//! Monday cannot starve a quiet Saturday — and the per-period summaries are combined
//! by a bounded hierarchical REDUCE. Coverage is structural: every period with frames
//! is summarized, and the per-period floor guarantees each contributes.
//!
//! Two retrieval paths feed the same map→reduce machinery (`docs/0.2.0.md` PR6):
//! - **coverage** (Daily / Weekly / Custom *without* a prompt): one group per active
//!   period, each evenly sampled across its own time window
//!   ([`Store::sample_frames_in_range`]);
//! - **relevance** (Custom *with* a prompt): the most relevant frames in the range via
//!   [`Store::hybrid_search`], grouped into token-budget batches.
//!
//! Grounding is always `content_text` (via [`Store::ocr_texts`], never raw text); the
//! report cites the frames the model actually read; empty ranges produce honest
//! no-evidence output with **no** sidecar call.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use traits::{
    AnswerOpts, AnswerProvider, ReportConfig, ReportMode, ReportOutput, ReportRange,
    RetrievedChunk, SearchHit, SearchQuery, Store, TimeRange,
};

const DAY_MS: i64 = 86_400_000;

/// Coverage floor: every active period contributes at least this many frames, even
/// when the global cap (`reports.weekly_top_k`) would round its quota below it. Keeps
/// long ranges from collapsing coverage. A structural constant (not a user setting) —
/// bounded above by [`MAX_PERIODS`] so the floor can't blow past the global cap.
const MIN_FRAMES_PER_PERIOD: u32 = 3;
/// Children combined per parent at each hierarchical reduce level.
const REDUCE_FANOUT: usize = 6;
/// Cap on the period grid for long Custom ranges — beyond this the period widens to
/// multi-day buckets so the pass count stays bounded on weak hardware.
const MAX_PERIODS: u32 = 31;
/// Hard ceiling on total sidecar passes (map + reduce + final). If a pathological
/// density would exceed it, the report stops coarsening rather than adding calls and
/// is flagged `truncated` (honest framing).
const MAX_REPORT_PASSES: u32 = 64;
/// Cap on cited frame ids returned (the UI renders a subset anyway); the honest
/// `frames_summarized` count still reflects the full union.
const MAX_REPORT_CITATIONS: usize = 200;

/// Fallback answer context window used only when the provider doesn't report its own
/// (e.g. a test fake) — matches the answer lane's `sidecar.ctx_size` default. The live
/// budget comes from [`AnswerProvider::answer_context_budget`] so a user-lowered
/// `ctx_size` is respected. A heuristic for the reduce-fits gate and batch splitting
/// only — the inference crate's `build_summary_messages` is the hard bound that actually
/// prevents an overflowing prompt.
const REPORT_CTX_TOKENS: usize = 8192;
const REPORT_TEMPLATE_OVERHEAD: usize = 96;
const REPORT_ID_FRAMING: usize = 6;

/// MAP step: summarize one slice (period / batch) into a terse paragraph.
const MAP_SYSTEM_PROMPT: &str = "You summarize one slice of the user's screen activity \
from the provided content snippets, each tagged with its source frame id. Write a \
short, factual paragraph of what the user worked on and saw in this slice. Use ONLY \
the snippets — do not invent activity. Be terse; this will be combined with other \
slices.";

/// REDUCE step: combine several slice summaries into one, preserving time order.
const REDUCE_SYSTEM_PROMPT: &str = "You combine several section summaries of the user's \
screen activity into a single coherent, factual summary. Merge overlapping themes, \
keep the chronological order, and use ONLY the provided summaries — do not invent \
activity.";

/// FINAL step: write the user-facing report from snippets and/or section summaries.
const FINAL_SYSTEM_PROMPT: &str = "You write the user's recall report from the provided \
material (content snippets and/or dated section summaries) about their screen activity \
over a time range. Produce clear, well-structured markdown: a short overview, then the \
key activities and topics grouped sensibly and in time order. Use ONLY the provided \
material and be specific. If it shows little or no evidence, say so plainly rather \
than inventing activity.";

/// A callback the orchestrator invokes before each pass so the command can emit a
/// `report_progress` event. Kept impl-agnostic (the kernel never depends on Tauri):
/// `(stage_label, done, total)`.
pub type ReportProgress<'a> = dyn Fn(&str, u32, u32) + Send + Sync + 'a;

/// One slice of work fed to the MAP step: a human label plus its content chunks.
struct Group {
    label: String,
    chunks: Vec<RetrievedChunk>,
}

/// Generates a recall report over `[start, end)` (`03 §8b`). `range`/`prompt` select
/// the retrieval path and framing; `cfg` carries the settings-derived knobs. Emits
/// progress via `progress` and cooperatively cancels (between passes) when `cancel`
/// is set. Returns the assembled [`ReportOutput`] (markdown + citations + auditable
/// coverage/cost metadata). Errors (a sidecar failure, or cancellation) propagate as
/// `Err`; the command maps them.
#[allow(clippy::too_many_arguments)]
pub async fn generate_report(
    store: &dyn Store,
    answer: &dyn AnswerProvider,
    range: ReportRange,
    start: i64,
    end: i64,
    prompt: Option<&str>,
    cfg: ReportConfig,
    progress: Option<&ReportProgress<'_>>,
    cancel: &AtomicBool,
) -> Result<ReportOutput> {
    let prompt = prompt.map(str::trim).filter(|p| !p.is_empty());
    let opts = AnswerOpts {
        thinking: false,
        max_tokens: cfg.reply_budget,
    };
    // Budget the batch/reduce planning against the provider's *actual* answer-lane context
    // window — the user can lower `sidecar.ctx_size`, and assuming a larger window would let
    // the planner pack summaries the sidecar then silently truncates. Falls back to the
    // pinned default for providers that don't report one (e.g. the test fake).
    let ctx_tokens = answer
        .answer_context_budget()
        .map(|c| c as usize)
        .unwrap_or(REPORT_CTX_TOKENS);

    // --- Grouping: relevance batches (prompt) vs coverage grid (time) ---
    let (groups, periods_total, total_in_range) = if let Some(p) = prompt {
        let hits = store
            .hybrid_search(&SearchQuery {
                text: p.to_string(),
                limit: cfg.weekly_top_k,
                time_range: Some(TimeRange { start, end }),
                include_chrome: false,
            })
            .await?;
        let retrieved = hits.len() as u64;
        let chunks = hydrate_hits(store, hits).await?;
        let batches = split_chunks_into_batches(chunks, cfg.reply_budget, ctx_tokens);
        let groups: Vec<Group> = batches
            .into_iter()
            .enumerate()
            .map(|(i, chunks)| Group {
                label: format!("part {}", i + 1),
                chunks,
            })
            .collect();
        let periods_total = groups.len() as u32;
        (groups, periods_total, retrieved)
    } else {
        let grid = grid_size(start, end, MAX_PERIODS);
        let buckets = store.timeline_buckets(start, end, grid).await?;
        let total_in_range: u64 = buckets.iter().map(|b| u64::from(b.count)).sum();
        let counts: Vec<u32> = buckets.iter().map(|b| b.count).collect();
        let quotas = plan_depth(&counts, cfg.daily_top_k, cfg.weekly_top_k);
        // Sample each active period's frames (one windowed query per period — inherent to
        // per-period coverage), then hydrate **all** of them with a single bulk `ocr_texts`
        // read rather than one query per period (avoids an N+1 over the grid).
        let mut periods_frames = Vec::with_capacity(buckets.len());
        for (b, quota) in buckets.iter().zip(quotas) {
            periods_frames.push(store.sample_frames_in_range(b.start, b.end, quota).await?);
        }
        let all_ids: Vec<i64> = periods_frames
            .iter()
            .flatten()
            .map(|f| f.frame_id)
            .collect();
        let texts = store.ocr_texts(&all_ids).await.unwrap_or_default();
        let mut groups = Vec::new();
        for (b, frames) in buckets.iter().zip(periods_frames) {
            let day = ((b.start - start) / DAY_MS + 1).max(1);
            let chunks: Vec<RetrievedChunk> = frames
                .into_iter()
                .filter_map(|f| {
                    texts
                        .get(&f.frame_id)
                        .filter(|t| !t.trim().is_empty())
                        .map(|t| RetrievedChunk {
                            frame_id: f.frame_id,
                            text: t.clone(),
                            score: 0.0,
                            captured_at: f.captured_at,
                        })
                })
                .collect();
            if !chunks.is_empty() {
                groups.push(Group {
                    label: format!("day {day}"),
                    chunks,
                });
            }
        }
        (groups, grid, total_in_range)
    };

    let frames_sampled: usize = groups.iter().map(|g| g.chunks.len()).sum();
    if frames_sampled == 0 {
        return Ok(empty_output(periods_total));
    }
    let mut truncated = (frames_sampled as u64) < total_in_range;
    let periods_covered = groups.len() as u32;

    // --- Single-pass fast path (Daily common case / small ranges) ---
    if groups.len() <= 1 || frames_sampled <= cfg.map_reduce_min_frames as usize {
        check_cancel(cancel)?;
        emit(progress, "Writing report", 1, 1);
        let mut all: Vec<RetrievedChunk> = groups.into_iter().flat_map(|g| g.chunks).collect();
        if prompt.is_none() {
            all.sort_by_key(|c| c.captured_at); // coverage report narrates a timeline
        }
        let (markdown, cited) = answer
            .summarize(
                FINAL_SYSTEM_PROMPT,
                &final_instruction(range, prompt),
                &all,
                opts,
            )
            .await?;
        let (cited, summarized) = cap_citations(cited);
        return Ok(ReportOutput {
            markdown,
            cited_frame_ids: cited,
            mode: ReportMode::SinglePass,
            periods_total,
            periods_covered,
            frames_sampled: frames_sampled as u32,
            frames_summarized: summarized,
            passes: 1,
            truncated,
        });
    }

    // --- MAP: one summarize pass per group ---
    let group_count = groups.len() as u32;
    let mut nodes: Vec<String> = Vec::new();
    let mut union: Vec<i64> = Vec::new();
    let mut passes: u32 = 0;
    for (i, g) in groups.into_iter().enumerate() {
        check_cancel(cancel)?;
        emit(
            progress,
            &format!("Summarizing {} of {}", i + 1, group_count),
            i as u32 + 1,
            group_count,
        );
        let instruction = format!("Summarize {} of this range.", g.label);
        let (text, cited) = answer
            .summarize(MAP_SYSTEM_PROMPT, &instruction, &g.chunks, opts)
            .await?;
        passes += 1;
        if !text.trim().is_empty() {
            for id in cited {
                if !union.contains(&id) {
                    union.push(id);
                }
            }
            nodes.push(format!("{}: {}", g.label, text.trim()));
        }
        if passes >= MAX_REPORT_PASSES {
            truncated = true;
            break;
        }
    }
    if nodes.is_empty() {
        return Ok(empty_output(periods_total));
    }

    // --- REDUCE: bounded hierarchical fan-in, time order preserved ---
    while nodes.len() > 1 && !fits_single_pass(&nodes, cfg.reply_budget, ctx_tokens) {
        if passes >= MAX_REPORT_PASSES {
            truncated = true;
            break;
        }
        let mut next = Vec::with_capacity(nodes.len() / REDUCE_FANOUT + 1);
        for chunk_group in nodes.chunks(REDUCE_FANOUT) {
            // A trailing group of one needs no combining — pass it through untouched
            // rather than spend a model call summarizing a single summary.
            if chunk_group.len() == 1 {
                next.push(chunk_group[0].clone());
                continue;
            }
            check_cancel(cancel)?;
            emit(progress, "Combining summaries", passes, MAX_REPORT_PASSES);
            let (text, _) = answer
                .summarize(
                    REDUCE_SYSTEM_PROMPT,
                    "",
                    &nodes_as_chunks(chunk_group),
                    opts,
                )
                .await?;
            passes += 1;
            next.push(text.trim().to_string());
            if passes >= MAX_REPORT_PASSES {
                truncated = true;
                break;
            }
        }
        nodes = next;
        if truncated {
            break;
        }
    }

    // --- FINAL: the user-facing report ---
    check_cancel(cancel)?;
    emit(progress, "Writing report", group_count, group_count);
    let (markdown, _) = answer
        .summarize(
            FINAL_SYSTEM_PROMPT,
            &final_instruction(range, prompt),
            &nodes_as_chunks(&nodes),
            opts,
        )
        .await?;
    passes += 1;

    let (cited, summarized) = cap_citations(union);
    Ok(ReportOutput {
        markdown,
        cited_frame_ids: cited,
        mode: ReportMode::MapReduce,
        periods_total,
        periods_covered,
        frames_sampled: frames_sampled as u32,
        frames_summarized: summarized,
        passes,
        truncated,
    })
}

/// Hydrate relevance hits with their `content_text` in one bulk read (`Store::ocr_texts`
/// returns `content_text`, never raw), falling back to the FTS snippet when a frame's
/// text is unavailable; relevance order (best-first) is preserved.
async fn hydrate_hits(store: &dyn Store, hits: Vec<SearchHit>) -> Result<Vec<RetrievedChunk>> {
    let ids: Vec<i64> = hits.iter().map(|h| h.frame_id).collect();
    let texts = store.ocr_texts(&ids).await.unwrap_or_default();
    Ok(hits
        .into_iter()
        .filter_map(|h| {
            let text = texts.get(&h.frame_id).cloned().unwrap_or(h.snippet);
            if text.trim().is_empty() {
                None
            } else {
                Some(RetrievedChunk {
                    frame_id: h.frame_id,
                    text,
                    score: h.score,
                    captured_at: h.captured_at,
                })
            }
        })
        .collect())
}

/// Per-active-period frame quota (pure — the coverage guarantee lives here). Aims for
/// `daily_top_k` frames per period, scaling down to fit the global `weekly_top_k` cap
/// when there are many periods, but never below [`MIN_FRAMES_PER_PERIOD`] (the floor
/// wins so every active period is represented). Each quota is capped at the period's
/// actual frame count.
fn plan_depth(period_counts: &[u32], daily_top_k: u32, weekly_top_k: u32) -> Vec<u32> {
    let a = period_counts.len() as u32;
    if a == 0 {
        return Vec::new();
    }
    let per_period = if daily_top_k.saturating_mul(a) <= weekly_top_k {
        daily_top_k
    } else {
        (weekly_top_k / a).max(MIN_FRAMES_PER_PERIOD)
    };
    let target = per_period.max(MIN_FRAMES_PER_PERIOD).max(1);
    period_counts.iter().map(|&c| target.min(c)).collect()
}

/// The period-grid size: one bucket per calendar day, capped at `max_periods` (a long
/// Custom range widens each period to multi-day buckets). Pure.
fn grid_size(start: i64, end: i64, max_periods: u32) -> u32 {
    if end <= start {
        return 0;
    }
    let span = end - start;
    let n_days = ((span + DAY_MS - 1) / DAY_MS) as u32; // ceil, >= 1
    n_days.clamp(1, max_periods.max(1))
}

/// Whether the section summaries fit a single (final) pass within `ctx_tokens` (the
/// provider's effective context window). Conservative heuristic; the inference budgeter
/// is the hard bound.
fn fits_single_pass(nodes: &[String], reply_budget: u32, ctx_tokens: usize) -> bool {
    let reserve =
        reply_budget as usize + est_tokens(FINAL_SYSTEM_PROMPT) + REPORT_TEMPLATE_OVERHEAD;
    let budget = ctx_tokens.saturating_sub(reserve);
    let used: usize = nodes
        .iter()
        .map(|n| est_tokens(n) + REPORT_ID_FRAMING)
        .sum();
    used <= budget
}

/// Splits relevance chunks (prompt path) into batches each fitting one map pass within
/// `ctx_tokens` (the provider's effective context window). Pure.
fn split_chunks_into_batches(
    chunks: Vec<RetrievedChunk>,
    reply_budget: u32,
    ctx_tokens: usize,
) -> Vec<Vec<RetrievedChunk>> {
    let reserve = reply_budget as usize + est_tokens(MAP_SYSTEM_PROMPT) + REPORT_TEMPLATE_OVERHEAD;
    let per_batch = ctx_tokens.saturating_sub(reserve).max(1);
    let mut batches: Vec<Vec<RetrievedChunk>> = Vec::new();
    let mut cur: Vec<RetrievedChunk> = Vec::new();
    let mut spent = 0usize;
    for c in chunks {
        if c.text.trim().is_empty() {
            continue;
        }
        let cost = est_tokens(c.text.trim()) + REPORT_ID_FRAMING;
        if !cur.is_empty() && spent + cost > per_batch {
            batches.push(std::mem::take(&mut cur));
            spent = 0;
        }
        cur.push(c);
        spent += cost;
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    batches
}

/// Wraps section-summary strings as pseudo-chunks for a reduce/final pass (the frame
/// tag is a harmless list marker; real citations are tracked separately).
fn nodes_as_chunks(nodes: &[String]) -> Vec<RetrievedChunk> {
    nodes
        .iter()
        .enumerate()
        .map(|(i, t)| RetrievedChunk {
            frame_id: i as i64,
            text: t.clone(),
            score: 0.0,
            captured_at: i as i64,
        })
        .collect()
}

/// Caps the cited list for the wire, returning `(capped, true_count)` so the honest
/// `frames_summarized` reflects the full union even when the list is truncated.
fn cap_citations(mut union: Vec<i64>) -> (Vec<i64>, u32) {
    let total = union.len() as u32;
    union.truncate(MAX_REPORT_CITATIONS);
    (union, total)
}

fn final_instruction(range: ReportRange, prompt: Option<&str>) -> String {
    let base = match range {
        ReportRange::Daily => "Write the recall report for today.",
        ReportRange::Weekly => "Write the recall report for the last 7 days.",
        ReportRange::Custom => "Write the recall report for the selected range.",
    };
    match prompt {
        Some(p) => format!("{base} Focus on: {p}"),
        None => base.to_string(),
    }
}

fn empty_output(periods_total: u32) -> ReportOutput {
    ReportOutput {
        markdown: "No screen activity was captured for this range — there is nothing to summarize."
            .to_string(),
        cited_frame_ids: Vec::new(),
        mode: ReportMode::Empty,
        periods_total,
        periods_covered: 0,
        frames_sampled: 0,
        frames_summarized: 0,
        passes: 0,
        truncated: false,
    }
}

/// Conservative bytes-per-token estimate (mirrors the inference budgeter's upper
/// bound) — only a heuristic for the reduce-fits gate; the real bound is enforced in
/// the inference crate.
fn est_tokens(text: &str) -> usize {
    text.len() / 2 + 1
}

fn emit(progress: Option<&ReportProgress<'_>>, stage: &str, done: u32, total: u32) {
    if let Some(p) = progress {
        p(stage, done, total);
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(anyhow!("report cancelled"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use store::SqliteStore;
    use traits::{AnswerDelta, NewFrame, OcrResult};

    // ---- Pure-function tests (no store / no sidecar) ----

    #[test]
    fn grid_size_is_one_per_day_capped_at_max() {
        assert_eq!(grid_size(0, DAY_MS, MAX_PERIODS), 1); // daily
        assert_eq!(grid_size(0, 7 * DAY_MS, MAX_PERIODS), 7); // weekly
        assert_eq!(grid_size(0, 60 * DAY_MS, MAX_PERIODS), MAX_PERIODS); // widened
        assert_eq!(grid_size(10, 10, MAX_PERIODS), 0); // degenerate
    }

    #[test]
    fn plan_depth_gives_every_active_period_its_budget() {
        // 7 days, budget fits: each active period gets daily_top_k (capped at count).
        let counts = [50u32, 4, 30, 8, 12, 20, 40];
        let q = plan_depth(&counts, 40, 200);
        for (c, quota) in counts.iter().zip(&q) {
            assert!(*quota >= MIN_FRAMES_PER_PERIOD.min(*c));
            assert!(*quota <= *c, "quota {quota} must not exceed count {c}");
        }
        // A dense day cannot consume a quiet day's allotment (independent quotas).
        assert_eq!(q[1], 4); // quiet day capped at its 4 frames
    }

    #[test]
    fn plan_depth_floor_wins_over_global_cap_on_long_ranges() {
        // Many active periods, tight global cap: floor still guarantees >= 1 each.
        let counts = vec![10u32; 90];
        let q = plan_depth(&counts, 40, 200);
        assert_eq!(q.len(), 90);
        assert!(
            q.iter().all(|&x| x >= 1),
            "every active period must get at least one frame"
        );
        // The floor (3) wins even though 90 * 3 > the 200 global cap.
        assert!(q.iter().all(|&x| x >= MIN_FRAMES_PER_PERIOD));
    }

    #[test]
    fn fits_single_pass_detects_overflow() {
        let small = vec!["short summary".to_string(); 3];
        assert!(fits_single_pass(&small, 512, REPORT_CTX_TOKENS));
        let huge = vec!["x".repeat(8_000); 7]; // ~28k tokens >> 8192
        assert!(!fits_single_pass(&huge, 512, REPORT_CTX_TOKENS));
        // A smaller provider context tightens the gate: summaries that fit 8192 no longer
        // fit a lowered window (mirrors a user-lowered `sidecar.ctx_size`).
        let medium = vec!["lorem ipsum dolor ".repeat(60); 4];
        assert!(fits_single_pass(&medium, 512, REPORT_CTX_TOKENS));
        assert!(!fits_single_pass(&medium, 512, 1_024));
    }

    #[test]
    fn split_chunks_batches_every_chunk_without_dropping() {
        let chunks: Vec<RetrievedChunk> = (0..50)
            .map(|i| RetrievedChunk {
                frame_id: i,
                text: "lorem ipsum ".repeat(40),
                score: 0.0,
                captured_at: i,
            })
            .collect();
        let batches = split_chunks_into_batches(chunks, 512, REPORT_CTX_TOKENS);
        let total: usize = batches.iter().map(|b| b.len()).sum();
        assert_eq!(total, 50, "no chunk dropped");
        assert!(
            batches.len() > 1,
            "large input splits into multiple batches"
        );
    }

    // ---- Orchestrator tests (real in-memory store + fake answer provider) ----

    /// Records its calls and echoes a configurable-size summary, citing exactly the
    /// frames it was given — so the test can assert coverage + citation propagation
    /// without a real model. `big` forces large summaries to exercise the reduce tree.
    struct FakeAnswer {
        calls: Arc<AtomicUsize>,
        big: bool,
    }

    #[async_trait]
    impl AnswerProvider for FakeAnswer {
        async fn answer(
            &self,
            _query: &str,
            _context: &[RetrievedChunk],
            _opts: AnswerOpts,
            _tx: tokio::sync::mpsc::Sender<AnswerDelta>,
        ) -> Result<()> {
            Ok(())
        }
        async fn summarize(
            &self,
            _system: &str,
            instruction: &str,
            context: &[RetrievedChunk],
            _opts: AnswerOpts,
        ) -> Result<(String, Vec<i64>)> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let cited: Vec<i64> = context.iter().map(|c| c.frame_id).collect();
            let body = if self.big {
                "lorem ".repeat(700) // ~4.2k chars → ~2.1k tokens per node
            } else {
                format!("[{} frames]", context.len())
            };
            Ok((format!("{instruction} {body}"), cited))
        }
    }

    fn cfg() -> ReportConfig {
        ReportConfig {
            daily_top_k: 40,
            weekly_top_k: 200,
            map_reduce_min_frames: 20,
            reply_budget: 512,
        }
    }

    async fn seed_day(store: &SqliteStore, day_start: i64, n: usize) -> Vec<i64> {
        let mut ids = Vec::new();
        for i in 0..n {
            let at = day_start + (i as i64) * 1000;
            let fid = store
                .insert_frame(NewFrame {
                    captured_at: at,
                    monitor_index: 0,
                    width: 1920,
                    height: 1080,
                    image_path: format!("frames/{at}.jpg"),
                    content_hash: format!("h{at}"),
                    app_hint: None,
                    window_title: None,
                    browser_url: None,
                })
                .await
                .unwrap();
            store
                .insert_ocr(
                    fid,
                    OcrResult {
                        text: format!("content for frame {fid} at {at}"),
                        mean_confidence: -1.0,
                        engine: "test".to_string(),
                        spans: Vec::new(),
                    },
                )
                .await
                .unwrap();
            ids.push(fid);
        }
        ids
    }

    #[tokio::test]
    async fn empty_range_is_honest_with_no_sidecar_call() {
        let store = SqliteStore::open_in_memory().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let answer = FakeAnswer {
            calls: calls.clone(),
            big: false,
        };
        let cancel = AtomicBool::new(false);
        let out = generate_report(
            &store,
            &answer,
            ReportRange::Daily,
            0,
            DAY_MS,
            None,
            cfg(),
            None,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(out.mode, ReportMode::Empty);
        assert_eq!(out.passes, 0);
        assert!(out.cited_frame_ids.is_empty());
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "no model call on empty range"
        );
    }

    #[tokio::test]
    async fn daily_small_range_uses_single_pass() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed_day(&store, 0, 5).await; // 5 frames in one day, < map_reduce_min_frames
        let calls = Arc::new(AtomicUsize::new(0));
        let answer = FakeAnswer {
            calls: calls.clone(),
            big: false,
        };
        let cancel = AtomicBool::new(false);
        let out = generate_report(
            &store,
            &answer,
            ReportRange::Daily,
            0,
            DAY_MS,
            None,
            cfg(),
            None,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(out.mode, ReportMode::SinglePass);
        assert_eq!(out.passes, 1);
        assert_eq!(out.frames_summarized, 5);
    }

    #[tokio::test]
    async fn weekly_covers_every_active_day_and_cites_first_and_last() {
        let store = SqliteStore::open_in_memory().unwrap();
        // 7 days; a dense Monday and a quiet Saturday, each day active.
        let mut first_day = Vec::new();
        let mut last_day = Vec::new();
        for d in 0..7i64 {
            let n = if d == 0 {
                60
            } else if d == 5 {
                4
            } else {
                15
            };
            let ids = seed_day(&store, d * DAY_MS, n).await;
            match d {
                0 => first_day = ids,
                6 => last_day = ids,
                _ => {}
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let answer = FakeAnswer {
            calls: calls.clone(),
            big: false,
        };
        let cancel = AtomicBool::new(false);
        let out = generate_report(
            &store,
            &answer,
            ReportRange::Weekly,
            0,
            7 * DAY_MS,
            None,
            cfg(),
            None,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(out.mode, ReportMode::MapReduce);
        assert_eq!(out.periods_total, 7);
        assert_eq!(out.periods_covered, 7, "every active day summarized");
        // Citation propagation: frames from BOTH the first and last day survive.
        assert!(
            first_day.iter().any(|id| out.cited_frame_ids.contains(id)),
            "first day must be cited"
        );
        assert!(
            last_day.iter().any(|id| out.cited_frame_ids.contains(id)),
            "last day must be cited (no trailing-day drop)"
        );
        // A dense Monday cannot starve a quiet Saturday: the dense day is sampled
        // down (capped near its share) while the quiet day keeps all 4 frames.
        assert!(out.frames_sampled <= cfg().weekly_top_k);
    }

    #[tokio::test]
    async fn reduce_overflow_preserves_all_days_via_hierarchical_reduce() {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut first_day = Vec::new();
        let mut last_day = Vec::new();
        for d in 0..7i64 {
            let ids = seed_day(&store, d * DAY_MS, 15).await;
            match d {
                0 => first_day = ids,
                6 => last_day = ids,
                _ => {}
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        // `big` summaries (7 × ~2k tokens) overflow one reduce pass → hierarchical.
        let answer = FakeAnswer {
            calls: calls.clone(),
            big: true,
        };
        let cancel = AtomicBool::new(false);
        let out = generate_report(
            &store,
            &answer,
            ReportRange::Weekly,
            0,
            7 * DAY_MS,
            None,
            cfg(),
            None,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(out.mode, ReportMode::MapReduce);
        // 7 map + 1 reduce (the 6-node group; the trailing single node passes through with
        // no call) + 1 final = 9 passes (hierarchical reduce engaged).
        assert!(
            out.passes >= 9,
            "hierarchical reduce ran: {} passes",
            out.passes
        );
        assert!(first_day.iter().any(|id| out.cited_frame_ids.contains(id)));
        assert!(
            last_day.iter().any(|id| out.cited_frame_ids.contains(id)),
            "no day dropped through the reduce tree"
        );
    }

    #[tokio::test]
    async fn cancellation_between_passes_returns_err() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed_day(&store, 0, 5).await;
        let answer = FakeAnswer {
            calls: Arc::new(AtomicUsize::new(0)),
            big: false,
        };
        let cancel = AtomicBool::new(true); // already cancelled
        let out = generate_report(
            &store,
            &answer,
            ReportRange::Daily,
            0,
            DAY_MS,
            None,
            cfg(),
            None,
            &cancel,
        )
        .await;
        assert!(out.is_err(), "a set cancel flag aborts before the pass");
    }
}
