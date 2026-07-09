//! CLI entry point for the dev-only segmentation validation harness (0.4.0 PR2).
//!
//! Phase A (this + export.rs):
//!   harness suggest-days [--db <path>]
//!   harness backup --to <dir> [--db <path>]      (D5 pre-migration snapshot; run FIRST)
//!   harness export --days <d1,d2,...> [--db <path>] [--out harness-data]
//! Phase B/C (score.rs, group.rs):
//!   harness replay | score [--algo grouped|micro] [--data harness-data] [--merge-gap ...] [...]
//!   harness sweep | stability [--data harness-data] [...]
//!
//! The default algorithm is the task-level GROUPED segmenter (`03 §7e` amendment `06` #27);
//! `--algo micro` selects the ungrouped baseline for the A/B comparison.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};

use harness::group::{segment_grouped, segment_grouped_detailed, CloseReason};
use harness::model::{GroupParams, SegParams, SessionSpan};
use harness::score::{
    pool, score_day, snap_label_boundaries, stability, sweep_1d, sweep_grid, Knob, OneDimPoint,
    SweepCell,
};
use harness::segmenter::{segment, FrameRow};
use harness::taxonomy::Taxonomy;
use harness::{data, export};

/// Parse `--name <i64>` with a default.
fn flag_i64(args: &[String], name: &str, default: i64) -> Result<i64> {
    match flag(args, name) {
        Some(v) => v
            .parse()
            .with_context(|| format!("{name} expects an integer, got {v:?}")),
        None => Ok(default),
    }
}

fn seg_params(args: &[String]) -> Result<SegParams> {
    Ok(SegParams {
        gap_close_ms: flag_i64(args, "--gap-close", 300)? * 1000,
        min_len_ms: flag_i64(args, "--min-len", 120)? * 1000,
    })
}

/// The macro grouping params (proposed `sessions.*` defaults; overridable per run).
fn group_params(args: &[String]) -> Result<GroupParams> {
    Ok(GroupParams {
        merge_gap_ms: flag_i64(args, "--merge-gap", 1500)? * 1000,
        absorb_max_ms: flag_i64(args, "--absorb-max", 1200)? * 1000,
        meeting_gap_ms: flag_i64(args, "--meeting-gap", 480)? * 1000,
        focus_min_len_ms: flag_i64(args, "--focus-min-len", 600)? * 1000,
        focus_min_density_fph: flag_i64(args, "--focus-density", 90)?,
    })
}

/// Whether to use the GROUPED segmenter (default) or the ungrouped `micro` baseline (`--algo`).
fn algo_grouped(args: &[String]) -> bool {
    !matches!(flag(args, "--algo"), Some("micro"))
}

/// Spans for the selected algorithm.
fn spans_for(
    grouped: bool,
    frames: &[FrameRow],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
) -> Vec<SessionSpan> {
    if grouped {
        segment_grouped(frames, tax, sp, gp)
    } else {
        segment(frames, tax, sp)
    }
}

fn frame_times(frames: &[FrameRow]) -> Vec<i64> {
    frames.iter().map(|f| f.captured_at).collect()
}

fn close_label(r: CloseReason) -> &'static str {
    match r {
        CloseReason::Gap => "gap",
        CloseReason::SustainedForeignIdentity => "switch",
        CloseReason::MeetingBandEdge => "meeting",
        CloseReason::EndOfInput => "eod",
    }
}

fn data_dir(args: &[String]) -> PathBuf {
    PathBuf::from(flag(args, "--data").unwrap_or("harness-data"))
}

fn usage() -> &'static str {
    "harness \u{2014} dev-only segmentation validation harness (0.4.0 PR2)\n\
     \n\
     USAGE:\n\
     \x20 harness suggest-days [--db <path>]\n\
     \x20 harness backup --to <dir> [--db <path>]        (D5 snapshot; run before any other live-DB command)\n\
     \x20 harness export --days <d1,d2,...> [--db <path>] [--out harness-data]\n\
     \x20 harness replay [--algo grouped|micro] [--data harness-data] [seg/group flags]\n\
     \x20 harness score  [--algo grouped|micro] [--data harness-data] [seg/group flags]\n\
     \x20 harness sweep | stability [--data harness-data] [seg/group flags]\n\
     \n\
     Group flags: --merge-gap <s> --absorb-max <s> --meeting-gap <s> --focus-min-len <s> --focus-density <fph>\n\
     Seg flags:   --gap-close <s> --min-len <s>   Scoring: --tolerance <s>\n\
     The live DB defaults to %APPDATA%\\app.screensearchv2c.desktop\\screensearch.db.\n"
}

/// Value of `--name <value>`, or `None`.
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn db_path(args: &[String]) -> Result<PathBuf> {
    if let Some(p) = flag(args, "--db") {
        return Ok(PathBuf::from(p));
    }
    export::default_db_path().context("APPDATA is unset; pass --db <path> to the live DB")
}

fn cmd_suggest_days(args: &[String]) -> Result<()> {
    let path = db_path(args)?;
    let conn = export::open_readonly(&path)?;
    let rows = export::survey(&conn)?;
    println!(
        "{:<12}  {:>8}  {:>5}  {:>10}  {:>12}  {:>5}",
        "day", "frames", "apps", "ai-title", "meeting-ttl", "marks"
    );
    for r in &rows {
        println!(
            "{:<12}  {:>8}  {:>5}  {:>10}  {:>12}  {:>5}",
            r.day, r.frames, r.distinct_apps, r.ai_title_frames, r.meeting_title_frames, r.marks
        );
    }
    println!(
        "\n{} day(s). Pick 5\u{2013}10 covering: a meeting-heavy day, a Claude Code day, a Codex \
         day, a browser-AI day, a mixed/fragmented day, plus one contiguous 2\u{2013}3-day stretch \
         (for the freeze-lookback stability check).\n\
         The ai-title / meeting-ttl columns are coarse window-title LIKE signals, not \
         authoritative recognition.",
        rows.len()
    );
    Ok(())
}

fn cmd_backup(args: &[String]) -> Result<()> {
    let to =
        flag(args, "--to").context("backup needs --to <dir> (outside the repo + app data dir)")?;
    let path = db_path(args)?;
    // Today's local date from the DB clock (no date library in the bin).
    let ymd = {
        let conn = export::open_readonly(&path)?;
        export::today_local(&conn)?
    };
    let report = export::backup(&path, Path::new(to), &ymd)?;
    println!("D5 backup written: {}", report.dest.display());
    println!(
        "PRAGMA integrity_check: {}",
        if report.integrity_ok { "ok" } else { "NOT ok" }
    );
    println!(
        "row counts \u{2014} source: {} frames / {} marks; copy: {} frames / {} marks (match: {})",
        report.src_frames,
        report.src_marks,
        report.dst_frames,
        report.dst_marks,
        report.src_frames == report.dst_frames && report.src_marks == report.dst_marks
    );
    if !report.integrity_ok {
        bail!("integrity_check did not return 'ok' \u{2014} do not rely on this backup");
    }
    Ok(())
}

fn cmd_export(args: &[String]) -> Result<()> {
    let days = flag(args, "--days").context("export needs --days <YYYY-MM-DD,...>")?;
    let out = PathBuf::from(flag(args, "--out").unwrap_or("harness-data"));
    let path = db_path(args)?;
    let conn = export::open_readonly(&path)?;
    for date in days.split(',').map(str::trim).filter(|d| !d.is_empty()) {
        let bounds = export::day_bounds(&conn, date)?;
        let dir = export::write_day(&out, date, bounds, &conn)?;
        println!("exported {date} -> {}", dir.display());
    }
    println!(
        "\nExports are git-ignored (harness-data/). Hand-label each day's labels.toml from its \
         digest.md, then run `harness score`."
    );
    Ok(())
}

fn cmd_replay(args: &[String]) -> Result<()> {
    let days = data::load_all(&data_dir(args))?;
    let tax = Taxonomy::seed();
    let sp = seg_params(args)?;
    let gp = group_params(args)?;
    let grouped = algo_grouped(args);
    for d in &days {
        if grouped {
            let sessions = segment_grouped_detailed(&d.frames, &tax, &sp, &gp);
            println!(
                "{}: {} candidate session(s) [grouped]",
                d.date,
                sessions.len()
            );
            for gs in &sessions {
                let s = &gs.span;
                println!(
                    "  [{:>6}s] {:<22} kind={:?} tool={} host={:?} frames={} close={}",
                    (s.end_ms - s.start_ms) / 1000,
                    s.context_key,
                    s.kind,
                    s.tool.as_deref().unwrap_or("-"),
                    s.host,
                    s.frame_count,
                    close_label(gs.close_reason)
                );
            }
        } else {
            let spans = segment(&d.frames, &tax, &sp);
            println!("{}: {} candidate session(s) [micro]", d.date, spans.len());
            for s in &spans {
                println!(
                    "  [{:>6}s] {:<24} kind={:?} tool={} host={:?} frames={}",
                    (s.end_ms - s.start_ms) / 1000,
                    s.context_key,
                    s.kind,
                    s.tool.as_deref().unwrap_or("-"),
                    s.host,
                    s.frame_count
                );
            }
        }
    }
    Ok(())
}

fn cmd_score(args: &[String]) -> Result<()> {
    let days = data::load_all(&data_dir(args))?;
    let tax = Taxonomy::seed();
    let sp = seg_params(args)?;
    let gp = group_params(args)?;
    let grouped = algo_grouped(args);
    let labeled: Vec<_> = days.iter().filter(|d| !d.labels.is_empty()).collect();
    if labeled.is_empty() {
        bail!(
            "no labeled days under {} \u{2014} fill in labels.toml first",
            data_dir(args).display()
        );
    }
    println!(
        "Scoring {} labeled day(s), algo={}, merge_gap {}s, absorb_max {}s, focus_min_len {}s, \
         density {}fph. Labels snapped to the nearest frame.",
        labeled.len(),
        if grouped { "grouped" } else { "micro" },
        gp.merge_gap_ms / 1000,
        gp.absorb_max_ms / 1000,
        gp.focus_min_len_ms / 1000,
        gp.focus_min_density_fph,
    );
    // Report at both the primary (120 s) and the secondary (180 s) tolerance.
    for tol in [120i64, 180] {
        let tol_ms = tol * 1000;
        println!("\n-- tolerance {tol}s --");
        println!(
            "{:<12}  {:>5} {:>5} {:>5}  {:>5} {:>5} {:>5}  {:>9}",
            "day", "pred", "lab", "match", "P", "R", "F1", "tool"
        );
        let mut day_scores = Vec::new();
        for d in &labeled {
            let spans = spans_for(grouped, &d.frames, &tax, &sp, &gp);
            let labels = snap_label_boundaries(&d.labels, &frame_times(&d.frames));
            let s = score_day(
                &d.date,
                &spans,
                &labels,
                d.day_start_ms,
                d.day_end_ms,
                tol_ms,
            );
            println!(
                "{:<12}  {:>5} {:>5} {:>5}  {:>5.2} {:>5.2} {:>5.2}  {:>4}/{:<4}",
                s.date,
                s.boundaries.predicted,
                s.boundaries.labeled,
                s.boundaries.matched,
                s.boundaries.precision(),
                s.boundaries.recall(),
                s.boundaries.f1(),
                s.tool_correct,
                s.tool_total
            );
            day_scores.push(s);
        }
        let (b, tc, tt) = pool(&day_scores);
        println!(
            "POOLED  P={:.3} R={:.3} F1={:.3} (matched {}/{} pred, {}/{} lab); tool {}/{} = {:.3}",
            b.precision(),
            b.recall(),
            b.f1(),
            b.matched,
            b.predicted,
            b.matched,
            b.labeled,
            tc,
            tt,
            if tt == 0 { 1.0 } else { tc as f64 / tt as f64 }
        );
    }
    Ok(())
}

/// Markdown row for one pooled `BoundaryScore` + pred count.
fn cell_row(mg: i64, am: i64, c: &SweepCell) -> String {
    format!(
        "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {} |\n",
        mg,
        am,
        c.boundary.precision(),
        c.boundary.recall(),
        c.boundary.f1(),
        c.tool_accuracy(),
        c.pred_sessions
    )
}

fn cmd_sweep(args: &[String]) -> Result<()> {
    let dir = data_dir(args);
    let days = data::load_all(&dir)?;
    let labeled: Vec<_> = days.into_iter().filter(|d| !d.labels.is_empty()).collect();
    if labeled.is_empty() {
        bail!(
            "no labeled days under {} \u{2014} fill in labels.toml first",
            dir.display()
        );
    }
    let tax = Taxonomy::seed();
    let tol_ms = flag_i64(args, "--tolerance", 120)? * 1000;
    let sp = seg_params(args)?;
    let gp = group_params(args)?;

    // Baseline: today's ungrouped segmenter through the same referee (the A/B reference).
    let base: Vec<_> = labeled
        .iter()
        .map(|d| {
            let spans = segment(&d.frames, &tax, &sp);
            let labels = snap_label_boundaries(&d.labels, &frame_times(&d.frames));
            (
                score_day(
                    &d.date,
                    &spans,
                    &labels,
                    d.day_start_ms,
                    d.day_end_ms,
                    tol_ms,
                ),
                spans.len(),
            )
        })
        .collect();
    let (bb, btc, btt) = pool(&base.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>());
    let base_pred: usize = base.iter().map(|(_, n)| n).sum();
    let base_acc = if btt == 0 {
        1.0
    } else {
        btc as f64 / btt as f64
    };

    // Stage A: the merge_gap x absorb_max grid through the grouped pipeline.
    let merge_gaps = [600, 900, 1200, 1500, 1800, 2700];
    let absorb_maxes = [300, 600, 900, 1200, 1800];
    let cells = sweep_grid(&labeled, &tax, &sp, &gp, &merge_gaps, &absorb_maxes, tol_ms);

    let mut md = String::new();
    md.push_str(&format!(
        "# Parameter sweep ({} labeled day(s), tolerance {}s)\n\n\
         Everything runs through the GROUPED pipeline (`03 section 7e` amendment). Columns: pooled \
         typed boundary P/R/F1, tool-recognition accuracy, and total predicted session count \
         (the mega-merge honesty column).\n\n\
         BASELINE (ungrouped `--algo micro`, same referee): F1 {:.3} (P {:.3} R {:.3}), tool acc \
         {:.3}, {} predicted sessions.\n\n",
        labeled.len(),
        tol_ms / 1000,
        bb.f1(),
        bb.precision(),
        bb.recall(),
        base_acc,
        base_pred,
    ));
    md.push_str("## Stage A: merge_gap x absorb_max\n\n");
    md.push_str("| merge_gap (s) | absorb_max (s) | P | R | F1 | tool acc | pred sessions |\n");
    md.push_str("|---|---|---|---|---|---|---|\n");
    for c in &cells {
        md.push_str(&cell_row(c.merge_gap_secs, c.absorb_max_secs, c));
    }
    if let Some(best) = cells
        .iter()
        .max_by(|a, b| a.boundary.f1().partial_cmp(&b.boundary.f1()).unwrap())
    {
        md.push_str(&format!(
            "\nBest cell: merge_gap={}s absorb_max={}s -> F1 {:.3}, tool acc {:.3}, {} sessions.\n",
            best.merge_gap_secs,
            best.absorb_max_secs,
            best.boundary.f1(),
            best.tool_accuracy(),
            best.pred_sessions
        ));
    }

    // Stage B: 1-D sweeps of the remaining knobs, at the base params.
    md.push_str("\n## Stage B: 1-D knob sweeps (others at base)\n");
    let dims: &[(Knob, &[i64])] = &[
        (Knob::MeetingGap, &[300, 420, 480, 600, 900]),
        (Knob::FocusMinLen, &[120, 300, 600, 900]),
        (Knob::FocusDensity, &[0, 30, 60, 90, 120, 180]),
        (Knob::GapClose, &[120, 180, 300, 600]),
        (Knob::MinLen, &[60, 120, 180, 300]),
        (Knob::IdentityQualify, &[60, 120, 300, 600]),
    ];
    for (knob, values) in dims {
        let pts = sweep_1d(&labeled, &tax, &sp, &gp, *knob, values, tol_ms);
        md.push_str(&format!(
            "\n### {} (flat -> propose as a named constant, not a setting)\n\n\
             | value | P | R | F1 | tool acc | pred |\n|---|---|---|---|---|---|\n",
            knob.label()
        ));
        for p in &pts {
            md.push_str(&one_dim_row(p));
        }
        md.push_str(&format!("\nverdict: {}\n", flat_or_sensitive(&pts)));
    }

    let out = write_report(&dir, "sweep.md", &md)?;
    print!("{md}");
    println!("\nwrote {}", out.display());
    Ok(())
}

fn one_dim_row(p: &OneDimPoint) -> String {
    format!(
        "| {} | {:.3} | {:.3} | {:.3} | {:.3} | {} |\n",
        p.value,
        p.boundary.precision(),
        p.boundary.recall(),
        p.boundary.f1(),
        p.tool_accuracy(),
        p.pred_sessions
    )
}

/// Classify a 1-D response as flat (max F1 spread < 0.02) or sensitive.
fn flat_or_sensitive(pts: &[OneDimPoint]) -> String {
    let f1s: Vec<f64> = pts.iter().map(|p| p.boundary.f1()).collect();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &v in &f1s {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if hi - lo < 0.02 {
        format!(
            "FLAT (F1 spread {:.3}) -> propose as a named constant",
            hi - lo
        )
    } else {
        format!(
            "SENSITIVE (F1 spread {:.3}) -> keep as a settings key",
            hi - lo
        )
    }
}

fn cmd_stability(args: &[String]) -> Result<()> {
    let dir = data_dir(args);
    let days = data::load_all(&dir)?;
    if days.is_empty() {
        bail!("no exported days under {}", dir.display());
    }
    let sp = seg_params(args)?;
    let gp = group_params(args)?;
    let lookbacks = [6 * 3600, 12 * 3600, 24 * 3600, 48 * 3600];
    let step_ms = flag_i64(args, "--cutoff-step", 3600)? * 1000;
    let pts = stability(&days, &Taxonomy::seed(), &sp, &gp, &lookbacks, step_ms);

    let mut md = String::new();
    md.push_str(&format!(
        "# Freeze-lookback stability ({} day(s), GROUPED pipeline, merge_gap {}s, absorb_max {}s, \
         cutoff step {}s)\n\n\
         For each candidate lookback, the max distance an older boundary moved (or how many \
         disappeared) between a truncated replay and the full replay. `stable` = boundaries \
         older than the lookback stop moving.\n\n\
         | lookback | max drift (s) | disappeared | stable |\n|---|---|---|---|\n",
        days.len(),
        gp.merge_gap_ms / 1000,
        gp.absorb_max_ms / 1000,
        step_ms / 1000
    ));
    for pt in &pts {
        md.push_str(&format!(
            "| {}h | {} | {} | {} |\n",
            pt.lookback_secs / 3600,
            pt.max_drift_ms / 1000,
            pt.disappeared,
            if pt.stable { "yes" } else { "no" }
        ));
    }
    match pts.iter().find(|p| p.stable) {
        Some(p) => md.push_str(&format!(
            "\nSmallest stable lookback on this sample: {}h (proposed default 24h).\n",
            p.lookback_secs / 3600
        )),
        None => md.push_str(
            "\nNo tested lookback was stable on this sample (see the caveat in specs/05).\n",
        ),
    }
    let out = write_report(&dir, "stability.md", &md)?;
    print!("{md}");
    println!("\nwrote {}", out.display());
    Ok(())
}

/// Write a report under `<data_dir>/reports/` (local-only, git-ignored).
fn write_report(data_dir: &Path, name: &str, body: &str) -> Result<PathBuf> {
    let dir = data_dir.join("reports");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    std::fs::write(&path, body)?;
    Ok(path)
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("suggest-days") => cmd_suggest_days(&args),
        Some("backup") => cmd_backup(&args),
        Some("export") => cmd_export(&args),
        Some("replay") => cmd_replay(&args),
        Some("score") => cmd_score(&args),
        Some("sweep") => cmd_sweep(&args),
        Some("stability") => cmd_stability(&args),
        None => {
            print!("{}", usage());
            Ok(())
        }
        Some(other) => {
            eprint!("{}", usage());
            bail!("unknown subcommand {other:?}");
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
