//! Environment doctor — the P0 smoke-check (`02 §5`).
//!
//! Reports whether the three runtime prerequisites are present on this machine:
//! the **WebView2** runtime (the Tauri shell), the **Vulkan** loader (GPU
//! inference, with CPU fallback), and **llama-server** (the inference sidecar,
//! installed on first model use). Run with `cargo run -p doctor`.
//!
//! This is a diagnostic: it never fails the build. WebView2 missing is reported
//! as FAIL (the app needs it); Vulkan/llama missing are WARN (CPU fallback / not
//! installed until P4).

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    {
        checks::run()
    }
    #[cfg(not(windows))]
    {
        eprintln!("doctor: ScreenSearch V2c is Windows-only; nothing to check here.");
        ExitCode::SUCCESS
    }
}

#[cfg(windows)]
mod checks {
    use std::process::ExitCode;

    #[derive(Clone, Copy)]
    enum Level {
        Ok,
        Warn,
        Fail,
    }

    fn print_line(level: Level, name: &str, detail: &str) {
        let tag = match level {
            Level::Ok => "[ OK ]",
            Level::Warn => "[WARN]",
            Level::Fail => "[FAIL]",
        };
        println!("{tag}  {name:<14}  {detail}");
    }

    pub fn run() -> ExitCode {
        println!("ScreenSearch V2c — environment doctor (P0 smoke-check)\n");

        let (wv_level, wv_detail) = check_webview2();
        print_line(wv_level, "WebView2", &wv_detail);

        let (vk_level, vk_detail) = check_vulkan();
        print_line(vk_level, "Vulkan", &vk_detail);

        let (ll_level, ll_detail) = check_llama_server();
        print_line(ll_level, "llama-server", &ll_detail);

        println!("\nNote: Vulkan/llama-server WARN is expected pre-P4 (CPU fallback exists;");
        println!("models + sidecar download on first use). Diagnostic only — does not fail CI.");

        ExitCode::SUCCESS
    }

    /// Look up the Evergreen WebView2 Runtime version in the registry.
    fn check_webview2() -> (Level, String) {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        // Stable client id of the Evergreen WebView2 Runtime.
        const CLIENT: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
        let candidates = [
            (
                HKEY_LOCAL_MACHINE,
                format!("SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT}"),
            ),
            (
                HKEY_LOCAL_MACHINE,
                format!("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT}"),
            ),
            (
                HKEY_CURRENT_USER,
                format!("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{CLIENT}"),
            ),
        ];

        for (hive, path) in candidates {
            let root = RegKey::predef(hive);
            if let Ok(key) = root.open_subkey(&path) {
                if let Ok(pv) = key.get_value::<String, _>("pv") {
                    if !pv.is_empty() && pv != "0.0.0.0" {
                        return (Level::Ok, format!("Evergreen Runtime v{pv}"));
                    }
                }
            }
        }
        (
            Level::Fail,
            "not found — install the WebView2 Runtime (ships with Win11)".to_string(),
        )
    }

    /// Try to load the Vulkan loader and resolve a core entry point.
    fn check_vulkan() -> (Level, String) {
        // SAFETY: loading a well-known system DLL by name and resolving a symbol;
        // we never call into it, only check presence of the loader.
        unsafe {
            match libloading::Library::new("vulkan-1.dll") {
                Ok(lib) => {
                    let sym: Result<libloading::Symbol<unsafe extern "system" fn() -> i32>, _> =
                        lib.get(b"vkEnumerateInstanceVersion\0");
                    if sym.is_ok() {
                        (
                            Level::Ok,
                            "vulkan-1.dll loadable (GPU acceleration available)".to_string(),
                        )
                    } else {
                        (
                            Level::Warn,
                            "vulkan-1.dll loaded but core symbol missing".to_string(),
                        )
                    }
                }
                Err(_) => (
                    Level::Warn,
                    "vulkan-1.dll not loadable — CPU fallback will be used".to_string(),
                ),
            }
        }
    }

    /// Look for `llama-server.exe` on PATH (it is downloaded on first model use).
    fn check_llama_server() -> (Level, String) {
        const EXE: &str = "llama-server.exe";
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let candidate = dir.join(EXE);
                if candidate.is_file() {
                    return (Level::Ok, format!("found at {}", candidate.display()));
                }
            }
        }
        (
            Level::Warn,
            "not on PATH — bundled/downloaded for the sidecar in P4".to_string(),
        )
    }
}
