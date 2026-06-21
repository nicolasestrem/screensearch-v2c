//! Model registry resolution (`MODEL_REGISTRY`, `03 §6`).
//!
//! Maps a `(lane, tier)` to the verified HuggingFace repo it downloads from, and to
//! the on-disk layout under the app-data models dir. Per-file GGUF/mmproj names are
//! **not** hardcoded (`MODEL_REGISTRY` is explicit about this — quant/mmproj names
//! must match the repo's current contents); instead [`resolve_spec`] scans the local
//! dir and picks the weights (a non-mmproj `*.gguf`, preferring `Q4_K_M`) plus, for
//! the vision lane, its **same-repo** projector (`mmproj*.gguf`). Pairing a vision
//! model with a foreign projector crashes `llama-server`, so the projector is always
//! taken from the same directory as the weights.

use std::path::{Path, PathBuf};

pub use traits::{ModelLane, ModelTier};

/// A resolved model the supervisor can launch (`03 §6`). `mmproj_path` is `Some` only
/// for the vision lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpec {
    pub lane: ModelLane,
    pub tier: ModelTier,
    pub gguf_path: PathBuf,
    pub mmproj_path: Option<PathBuf>,
    pub ngl: u32,
}

/// The HuggingFace source for one `(lane, tier)` (`MODEL_REGISTRY §1/§2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRepo {
    /// HuggingFace repo id (verified to exist, `MODEL_REGISTRY`).
    pub repo_id: &'static str,
    /// Whether this lane needs an mmproj projector (vision yes, answer no).
    pub needs_mmproj: bool,
}

/// The verified repo for a `(lane, tier)` (`MODEL_REGISTRY §1/§2`). Defaults on first
/// run: vision = Qwen3-VL-4B-Instruct, answer = Ministral-3-3B-Reasoning.
pub fn repo_for(lane: ModelLane, tier: ModelTier) -> ModelRepo {
    match (lane, tier) {
        (ModelLane::Vision, ModelTier::Default) => ModelRepo {
            repo_id: "Qwen/Qwen3-VL-4B-Instruct-GGUF",
            needs_mmproj: true,
        },
        (ModelLane::Vision, ModelTier::Quality) => ModelRepo {
            repo_id: "Qwen/Qwen3-VL-8B-Instruct-GGUF",
            needs_mmproj: true,
        },
        (ModelLane::Vision, ModelTier::Beta) => ModelRepo {
            repo_id: "jc-builds/Qwen3.5-9B-VLM-Q4_K_M-GGUF",
            needs_mmproj: true,
        },
        (ModelLane::Answer, ModelTier::Default) => ModelRepo {
            repo_id: "unsloth/Ministral-3-3B-Reasoning-2512-GGUF",
            needs_mmproj: false,
        },
        (ModelLane::Answer, ModelTier::Quality) => ModelRepo {
            repo_id: "unsloth/Qwen3-4B-Thinking-2507-GGUF",
            needs_mmproj: false,
        },
        (ModelLane::Answer, ModelTier::Beta) => ModelRepo {
            repo_id: "nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF",
            needs_mmproj: false,
        },
    }
}

fn lane_slug(lane: ModelLane) -> &'static str {
    match lane {
        ModelLane::Vision => "vision",
        ModelLane::Answer => "answer",
    }
}

fn tier_slug(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Default => "default",
        ModelTier::Quality => "quality",
        ModelTier::Beta => "beta",
    }
}

/// The local directory that holds a tier's downloaded files:
/// `<models_root>/<lane>/<tier>`. Stable across repo changes so a re-download lands
/// in the same place.
pub fn local_dir(models_root: &Path, lane: ModelLane, tier: ModelTier) -> PathBuf {
    models_root.join(lane_slug(lane)).join(tier_slug(tier))
}

fn is_gguf(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
}

fn file_name_lower(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Finds the weights GGUF in `dir`: a `*.gguf` that is **not** an mmproj projector,
/// preferring a `Q4_K_M` quant (the `MODEL_REGISTRY` default), else any non-mmproj
/// gguf (deterministic by name so re-resolution is stable).
pub fn find_gguf(dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_gguf(p) && !file_name_lower(p).starts_with("mmproj"))
        .collect();
    candidates.sort();
    candidates
        .iter()
        .find(|p| file_name_lower(p).contains("q4_k_m"))
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

/// Finds the projector GGUF in `dir` (`mmproj*.gguf`). Vision only.
pub fn find_mmproj(dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_gguf(p) && file_name_lower(p).starts_with("mmproj"))
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Resolves a launchable [`ModelSpec`] from already-downloaded files, or `None` if the
/// tier's directory is missing required files (the caller then triggers a download).
/// For vision, both weights and a same-repo projector must be present.
pub fn resolve_spec(
    models_root: &Path,
    lane: ModelLane,
    tier: ModelTier,
    ngl: u32,
) -> Option<ModelSpec> {
    let dir = local_dir(models_root, lane, tier);
    let gguf_path = find_gguf(&dir)?;
    let mmproj_path = if repo_for(lane, tier).needs_mmproj {
        Some(find_mmproj(&dir)?)
    } else {
        None
    };
    Some(ModelSpec {
        lane,
        tier,
        gguf_path,
        mmproj_path,
        ngl,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp dir for a test (no external tempfile dep).
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ssv2c-models-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn repo_mapping_matches_registry() {
        assert_eq!(
            repo_for(ModelLane::Vision, ModelTier::Default).repo_id,
            "Qwen/Qwen3-VL-4B-Instruct-GGUF"
        );
        assert!(repo_for(ModelLane::Vision, ModelTier::Default).needs_mmproj);
        assert_eq!(
            repo_for(ModelLane::Answer, ModelTier::Default).repo_id,
            "unsloth/Ministral-3-3B-Reasoning-2512-GGUF"
        );
        assert!(!repo_for(ModelLane::Answer, ModelTier::Default).needs_mmproj);
    }

    #[test]
    fn prefers_q4_k_m_and_excludes_mmproj() {
        let root = temp_dir("q4");
        let dir = local_dir(&root, ModelLane::Vision, ModelTier::Default);
        std::fs::create_dir_all(&dir).unwrap();
        touch(&dir.join("Qwen3-VL-4B-Instruct-Q8_0.gguf"));
        touch(&dir.join("Qwen3-VL-4B-Instruct-Q4_K_M.gguf"));
        touch(&dir.join("mmproj-Qwen3-VL-4B-Instruct-f16.gguf"));

        let gguf = find_gguf(&dir).unwrap();
        assert_eq!(
            gguf.file_name().unwrap(),
            "Qwen3-VL-4B-Instruct-Q4_K_M.gguf"
        );
        let mmproj = find_mmproj(&dir).unwrap();
        assert_eq!(
            mmproj.file_name().unwrap(),
            "mmproj-Qwen3-VL-4B-Instruct-f16.gguf"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn vision_resolution_requires_mmproj() {
        let root = temp_dir("vision");
        let dir = local_dir(&root, ModelLane::Vision, ModelTier::Default);
        std::fs::create_dir_all(&dir).unwrap();
        touch(&dir.join("model-Q4_K_M.gguf"));
        // No mmproj yet → incomplete, must not resolve.
        assert!(resolve_spec(&root, ModelLane::Vision, ModelTier::Default, 99).is_none());

        touch(&dir.join("mmproj-model.gguf"));
        let spec = resolve_spec(&root, ModelLane::Vision, ModelTier::Default, 99).unwrap();
        assert!(spec.mmproj_path.is_some());
        assert_eq!(spec.ngl, 99);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn answer_resolution_needs_no_mmproj() {
        let root = temp_dir("answer");
        let dir = local_dir(&root, ModelLane::Answer, ModelTier::Default);
        std::fs::create_dir_all(&dir).unwrap();
        touch(&dir.join("Ministral-3-3B-Reasoning-Q4_K_M.gguf"));

        let spec = resolve_spec(&root, ModelLane::Answer, ModelTier::Default, 50).unwrap();
        assert!(spec.mmproj_path.is_none());
        assert_eq!(spec.lane, ModelLane::Answer);

        let _ = std::fs::remove_dir_all(&root);
    }
}
