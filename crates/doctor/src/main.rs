//! Environment doctor CLI — the P0 smoke-check (`02 §5`).
//!
//! `cargo run -p doctor`         → human-readable report
//! `cargo run -p doctor -- --json` → machine-readable JSON (for CI / tooling)
//!
//! Diagnostic only: always exits 0. WebView2 missing is reported FAIL (the app
//! needs it); Vulkan/llama-server missing are WARN (CPU fallback / not installed
//! until P4). Logic lives in the `doctor` library so the app can reuse it.

use std::process::ExitCode;

use doctor::{Level, Report};

fn main() -> ExitCode {
    let json = std::env::args().skip(1).any(|a| a == "--json");
    let report = Report::run();

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("doctor: failed to serialize report: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print_text(&report);
    }

    ExitCode::SUCCESS
}

fn print_text(report: &Report) {
    println!("ScreenSearch V2c — environment doctor (P0 smoke-check)\n");
    for c in &report.checks {
        let tag = match c.level {
            Level::Ok => "[ OK ]",
            Level::Warn => "[WARN]",
            Level::Fail => "[FAIL]",
        };
        println!("{tag}  {:<14}  {}", c.name, c.detail);
    }
    println!("\nNote: Vulkan/llama-server WARN is expected pre-P4 (CPU fallback exists;");
    println!("models + sidecar download on first use). Diagnostic only — does not fail CI.");
    println!("Pass --json for machine-readable output.");
}
