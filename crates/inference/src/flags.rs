//! One-shot probe of the bundled `llama-server` to learn which memory-tuning flags it
//! accepts (`build_args` in [`crate::supervisor`] only emits verified syntax).
//!
//! The binary is the **latest** llama.cpp Windows Vulkan release fetched at runtime
//! (`download.rs`), not a pinned version, so flag spelling is genuinely version-variable:
//! older builds take `--flash-attn` as a bare boolean, newer ones as
//! `--flash-attn on|off|auto`, and quantized KV cache (`--cache-type-v`) is rejected
//! unless flash attention is active. We parse `--help` once at init and emit only what
//! the binary advertises, so a future auto-updated build that renames or drops a flag
//! degrades to "don't pass it" rather than wedging every sidecar spawn.

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// `CREATE_NO_WINDOW` — keep the throwaway `--help` probe from flashing a console.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How the bundled binary spells `--flash-attn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashAttnKind {
    /// The flag is absent from `--help` — never emit it.
    Unsupported,
    /// Bare boolean: `--flash-attn` with no value (older builds).
    BoolFlag,
    /// Takes a value: `--flash-attn on|off|auto` (newer builds).
    EnumOnOffAuto,
}

/// Which memory-tuning flags the bundled `llama-server` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidecarCaps {
    pub ctx_size: bool,
    pub cache_type_k: bool,
    pub cache_type_v: bool,
    pub flash_attn_kind: FlashAttnKind,
}

impl SidecarCaps {
    /// Fallback when the probe can't run: assume only `--ctx-size` (supported by
    /// llama.cpp for years and the single biggest VRAM lever), and treat the riskier
    /// flags as unavailable so we never emit KV quantization / flash attention a build
    /// might reject. Pinning the context alone still cuts memory substantially.
    pub fn conservative() -> Self {
        Self {
            ctx_size: true,
            cache_type_k: false,
            cache_type_v: false,
            flash_attn_kind: FlashAttnKind::Unsupported,
        }
    }
}

/// Probes `<binary> --help` once and returns the supported flags. Falls back to
/// [`SidecarCaps::conservative`] (with a warning) if the binary can't be run.
pub fn probe_caps(binary: &Path) -> SidecarCaps {
    match run_help(binary) {
        Some(help) => {
            let caps = parse_caps(&help);
            tracing::info!(?caps, "probed llama-server flag capabilities");
            caps
        }
        None => {
            tracing::warn!(
                binary = %binary.display(),
                "could not run llama-server --help; using a conservative flag set"
            );
            SidecarCaps::conservative()
        }
    }
}

/// Runs `<binary> --help` (no GPU, no model load) and returns its combined output, or
/// `None` if the process couldn't be spawned or printed nothing.
fn run_help(binary: &Path) -> Option<String> {
    let mut cmd = Command::new(binary);
    cmd.arg("--help");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().ok()?;
    // Some builds print usage to stderr; fold both streams together before parsing.
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (!text.trim().is_empty()).then_some(text)
}

/// Parses a `--help` dump into the supported-flag set. Pure (testable without a binary).
pub fn parse_caps(help: &str) -> SidecarCaps {
    let flash_attn_kind = if !help.contains("--flash-attn") {
        FlashAttnKind::Unsupported
    } else if help
        .lines()
        .filter(|l| l.contains("--flash-attn"))
        .any(flash_line_takes_value)
    {
        FlashAttnKind::EnumOnOffAuto
    } else {
        FlashAttnKind::BoolFlag
    };
    SidecarCaps {
        ctx_size: help.contains("--ctx-size"),
        cache_type_k: help.contains("--cache-type-k"),
        cache_type_v: help.contains("--cache-type-v"),
        flash_attn_kind,
    }
}

/// Whether a `--flash-attn` help line advertises a value placeholder (e.g.
/// `{on,off,auto}` / `[on|off|auto]`), marking the newer value-taking form. The token
/// hints are specific enough not to match the bare-boolean line's prose.
fn flash_line_takes_value(line: &str) -> bool {
    const VALUE_HINTS: &[&str] = &["{on", "[on", "<on", "on,off", "on|off", "on/off"];
    VALUE_HINTS.iter().any(|hint| line.contains(hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_value_taking_flash_attn() {
        // The current llama.cpp help advertises a value set for --flash-attn.
        let help = "\
  -c,    --ctx-size N            size of the prompt context (default: 4096)
  -fa,   --flash-attn {on,off,auto}  set Flash Attention use (default: 'auto')
         --cache-type-k TYPE     KV cache data type for K (default: f16)
         --cache-type-v TYPE     KV cache data type for V (default: f16)";
        let caps = parse_caps(help);
        assert!(caps.ctx_size);
        assert!(caps.cache_type_k);
        assert!(caps.cache_type_v);
        assert_eq!(caps.flash_attn_kind, FlashAttnKind::EnumOnOffAuto);
    }

    #[test]
    fn parses_legacy_boolean_flash_attn() {
        // Older builds: --flash-attn is a bare switch; --cache-type-* may be absent.
        let help = "\
  -c,    --ctx-size N            size of the prompt context
  -fa,   --flash-attn            enable Flash Attention";
        let caps = parse_caps(help);
        assert!(caps.ctx_size);
        assert!(!caps.cache_type_k);
        assert!(!caps.cache_type_v);
        assert_eq!(caps.flash_attn_kind, FlashAttnKind::BoolFlag);
    }

    #[test]
    fn detects_missing_flash_attn() {
        let help = "  -c, --ctx-size N   size of the prompt context";
        let caps = parse_caps(help);
        assert!(caps.ctx_size);
        assert_eq!(caps.flash_attn_kind, FlashAttnKind::Unsupported);
    }

    #[test]
    fn conservative_fallback_only_pins_context() {
        let caps = SidecarCaps::conservative();
        assert!(caps.ctx_size);
        assert!(!caps.cache_type_k);
        assert!(!caps.cache_type_v);
        assert_eq!(caps.flash_attn_kind, FlashAttnKind::Unsupported);
    }
}
