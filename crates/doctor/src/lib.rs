//! Environment smoke-check **library** (P0, `02 §5`).
//!
//! Pure, serializable result types plus the platform probes for the three runtime
//! prerequisites — **WebView2** (the shell), the **Vulkan** loader (GPU inference,
//! CPU fallback otherwise), and **llama-server** (the sidecar, installed on first
//! model use). Keeping the logic in a library (not just a `main`) lets the `doctor`
//! CLI, CI, and — later — the app's first-run/readiness panel share one source of
//! truth instead of re-implementing the checks.

use serde::Serialize;

/// Severity of a single check, ordered `Ok < Warn < Fail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

/// The result of one environment check.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub level: Level,
    pub detail: String,
}

/// A full environment report (all checks for this machine).
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// Run every environment check on the current machine.
    pub fn run() -> Self {
        #[cfg(windows)]
        {
            Self {
                checks: vec![win::webview2(), win::vulkan(), win::llama_server()],
            }
        }
        #[cfg(not(windows))]
        {
            Self {
                checks: vec![Check {
                    name: "platform".to_string(),
                    level: Level::Warn,
                    detail: "ScreenSearch V2c is Windows-only; nothing to check here.".to_string(),
                }],
            }
        }
    }

    /// The most severe level across all checks (`Ok` if there are none).
    pub fn worst(&self) -> Level {
        self.checks
            .iter()
            .map(|c| c.level)
            .max()
            .unwrap_or(Level::Ok)
    }
}

#[cfg(windows)]
mod win {
    use super::{Check, Level};

    /// Evergreen WebView2 Runtime version, from the registry.
    pub fn webview2() -> Check {
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
                        return Check {
                            name: "WebView2".to_string(),
                            level: Level::Ok,
                            detail: format!("Evergreen Runtime v{pv}"),
                        };
                    }
                }
            }
        }
        Check {
            name: "WebView2".to_string(),
            level: Level::Fail,
            detail: "not found — install the WebView2 Runtime (ships with Win11)".to_string(),
        }
    }

    /// Whether the Vulkan loader is present and exposes a core entry point.
    pub fn vulkan() -> Check {
        // Resolve the loader by absolute System32 path rather than bare name, so a
        // rogue `vulkan-1.dll` in the working/app directory can't be picked up via
        // the DLL search order (hijacking). vulkan-1.dll lives in System32.
        let dll = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
            .join("System32")
            .join("vulkan-1.dll");

        // SAFETY: loading a known system DLL from an absolute path and resolving a
        // symbol; we never call into it, only check presence of the loader.
        let (level, detail) = unsafe {
            match libloading::Library::new(&dll) {
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
        };
        Check {
            name: "Vulkan".to_string(),
            level,
            detail,
        }
    }

    /// Whether `llama-server.exe` is on PATH (downloaded on first model use).
    pub fn llama_server() -> Check {
        const EXE: &str = "llama-server.exe";
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let candidate = dir.join(EXE);
                if candidate.is_file() {
                    return Check {
                        name: "llama-server".to_string(),
                        level: Level::Ok,
                        detail: format!("found at {}", candidate.display()),
                    };
                }
            }
        }
        Check {
            name: "llama-server".to_string(),
            level: Level::Warn,
            detail: "not on PATH — bundled/downloaded for the sidecar in P4".to_string(),
        }
    }
}
