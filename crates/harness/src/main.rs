//! CLI entry point for the dev-only segmentation validation harness (0.4.0 PR2).
//!
//! Subcommands (built out across Tasks 5/8):
//!   harness suggest-days [--db <path>]
//!   harness backup --to <dir> [--db <path>]
//!   harness export --days <d1,d2,...> [--db <path>] [--out harness-data]
//!   harness replay    [--data harness-data] [--gap-close 300] [--min-len 120]
//!   harness score     [--data harness-data] [--tolerance 120] [--gap-close 300] [--min-len 120]
//!   harness sweep     [--data harness-data]
//!   harness stability [--data harness-data]

fn usage() -> String {
    "harness — dev-only segmentation validation harness (0.4.0 PR2)\n\
     \n\
     USAGE:\n\
     \x20 harness suggest-days [--db <path>]\n\
     \x20 harness backup --to <dir> [--db <path>]\n\
     \x20 harness export --days <d1,d2,...> [--db <path>] [--out harness-data]\n\
     \x20 harness replay    [--data harness-data] [--gap-close 300] [--min-len 120]\n\
     \x20 harness score     [--data harness-data] [--tolerance 120] [--gap-close 300] [--min-len 120]\n\
     \x20 harness sweep     [--data harness-data]\n\
     \x20 harness stability [--data harness-data]\n"
        .to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("suggest-days" | "backup" | "export" | "replay" | "score" | "sweep" | "stability") => {
            eprintln!("not yet implemented (scaffold — filled in Tasks 5/8)");
            std::process::exit(2);
        }
        _ => {
            print!("{}", usage());
        }
    }
}
