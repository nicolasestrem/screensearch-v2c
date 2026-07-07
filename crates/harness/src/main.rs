//! CLI entry point for the dev-only segmentation validation harness (0.4.0 PR2).
//!
//! Phase A (this + export.rs):
//!   harness suggest-days [--db <path>]
//!   harness backup --to <dir> [--db <path>]      (D5 pre-migration snapshot; run FIRST)
//!   harness export --days <d1,d2,...> [--db <path>] [--out harness-data]
//! Phase B/C (score.rs, Task 8):
//!   harness replay | score | sweep | stability [--data harness-data] [...]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};

use harness::model::SegParams;
use harness::score::{pool, score_day, stability, sweep};
use harness::segmenter::segment;
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
     \x20 harness replay | score | sweep | stability [--data harness-data] [...]   (Task 8)\n\
     \n\
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
    let p = seg_params(args)?;
    for d in &days {
        let spans = segment(&d.frames, &tax, &p);
        println!("{}: {} candidate session(s)", d.date, spans.len());
        for s in &spans {
            let dur = (s.end_ms - s.start_ms) / 1000;
            println!(
                "  [{:>6}s] {:<24} kind={:?} tool={} host={:?} frames={}",
                dur,
                s.context_key,
                s.kind,
                s.tool.as_deref().unwrap_or("-"),
                s.host,
                s.frame_count
            );
        }
    }
    Ok(())
}

fn cmd_score(args: &[String]) -> Result<()> {
    let days = data::load_all(&data_dir(args))?;
    let tax = Taxonomy::seed();
    let p = seg_params(args)?;
    let tol_ms = flag_i64(args, "--tolerance", 120)? * 1000;
    let labeled: Vec<_> = days.iter().filter(|d| !d.labels.is_empty()).collect();
    if labeled.is_empty() {
        bail!(
            "no labeled days under {} \u{2014} fill in labels.toml first",
            data_dir(args).display()
        );
    }
    println!(
        "Scoring {} labeled day(s) at tolerance {}s, gap_close {}s, min_len {}s.\n",
        labeled.len(),
        tol_ms / 1000,
        p.gap_close_ms / 1000,
        p.min_len_ms / 1000
    );
    println!(
        "{:<12}  {:>5} {:>5} {:>5}  {:>5} {:>5} {:>5}  {:>9}",
        "day", "pred", "lab", "match", "P", "R", "F1", "tool"
    );
    let mut day_scores = Vec::new();
    for d in &labeled {
        let spans = segment(&d.frames, &tax, &p);
        let s = score_day(
            &d.date,
            &spans,
            &d.labels,
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
        "\nPOOLED  boundaries P={:.3} R={:.3} F1={:.3} (matched {}/{} pred, {}/{} lab); \
         tool-recognition {}/{} = {:.3}",
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
    Ok(())
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
    let tol_ms = flag_i64(args, "--tolerance", 120)? * 1000;
    let gaps = [120, 180, 240, 300, 420, 600];
    let mins = [60, 120, 180];
    let cells = sweep(&labeled, &Taxonomy::seed(), &gaps, &mins, tol_ms);

    let mut md = String::new();
    md.push_str(&format!(
        "# Parameter sweep ({} labeled day(s), tolerance {}s)\n\n\
         Pooled boundary F1 and tool-recognition accuracy per (gap_close, min_len). \
         `*` marks degenerate cells (gap_close <= min_len).\n\n\
         | gap_close (s) | min_len (s) | P | R | F1 | tool acc | note |\n\
         |---|---|---|---|---|---|---|\n",
        labeled.len(),
        tol_ms / 1000
    ));
    for c in &cells {
        md.push_str(&format!(
            "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {} |\n",
            c.gap_close_secs,
            c.min_len_secs,
            c.boundary.precision(),
            c.boundary.recall(),
            c.boundary.f1(),
            c.tool_accuracy(),
            if c.degenerate { "* degenerate" } else { "" }
        ));
    }
    let best = cells
        .iter()
        .filter(|c| !c.degenerate)
        .max_by(|a, b| a.boundary.f1().partial_cmp(&b.boundary.f1()).unwrap());
    if let Some(b) = best {
        md.push_str(&format!(
            "\nBest non-degenerate cell: gap_close={}s min_len={}s -> F1 {:.3}, tool acc {:.3}.\n",
            b.gap_close_secs,
            b.min_len_secs,
            b.boundary.f1(),
            b.tool_accuracy()
        ));
    }
    let out = write_report(&dir, "sweep.md", &md)?;
    print!("{md}");
    println!("\nwrote {}", out.display());
    Ok(())
}

fn cmd_stability(args: &[String]) -> Result<()> {
    let dir = data_dir(args);
    let days = data::load_all(&dir)?;
    if days.is_empty() {
        bail!("no exported days under {}", dir.display());
    }
    let p = seg_params(args)?;
    let lookbacks = [6 * 3600, 12 * 3600, 24 * 3600, 48 * 3600];
    let step_ms = flag_i64(args, "--cutoff-step", 3600)? * 1000;
    let pts = stability(&days, &Taxonomy::seed(), &p, &lookbacks, step_ms);

    let mut md = String::new();
    md.push_str(&format!(
        "# Freeze-lookback stability ({} day(s), gap_close {}s, min_len {}s, cutoff step {}s)\n\n\
         For each candidate lookback, the max distance an older boundary moved (or how many \
         disappeared) between a truncated replay and the full replay. `stable` = boundaries \
         older than the lookback stop moving.\n\n\
         | lookback | max drift (s) | disappeared | stable |\n|---|---|---|---|\n",
        days.len(),
        p.gap_close_ms / 1000,
        p.min_len_ms / 1000,
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
        _ => {
            print!("{}", usage());
            Ok(())
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
