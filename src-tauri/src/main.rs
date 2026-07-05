// Prevent an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--version` / `-V`: print and exit **before** the Tauri builder (so the
    // single-instance plugin never initializes and this never focuses/steals the running
    // window). Used by the auto-update acceptance runbook to capture before/after versions
    // (0.3.2 PR2, #69; `docs/TESTING.md`). In a release build `windows_subsystem="windows"`
    // means no attached console, but shell **redirection still works** (`ScreenSearch.exe
    // --version > out.txt`), which is how the runbook captures it — do not "fix" this with
    // AttachConsole.
    if std::env::args()
        .skip(1)
        .any(|a| a == "--version" || a == "-V")
    {
        println!("ScreenSearch {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    screensearch_lib::run();
}
