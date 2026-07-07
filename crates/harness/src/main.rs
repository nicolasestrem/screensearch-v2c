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

use harness::export;

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

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("suggest-days") => cmd_suggest_days(&args),
        Some("backup") => cmd_backup(&args),
        Some("export") => cmd_export(&args),
        Some(c @ ("replay" | "score" | "sweep" | "stability")) => {
            bail!("`{c}` is implemented in Task 8 (score.rs) \u{2014} not available yet")
        }
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
