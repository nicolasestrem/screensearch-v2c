//! Runtime acquisition of the sidecar binary + GGUF models — Rust-only, no Python
//! (`01 §5`; the user opted into runtime auto-download).
//!
//! - The **binary**: the latest `llama.cpp` Windows x64 **Vulkan** release zip from
//!   GitHub, extracted under app-data (or overridden via `SSV2C_LLAMA_RELEASE_URL`).
//! - The **models**: the tier's GGUF weights (preferring `Q4_K_M`) plus, for vision,
//!   the **same-repo** `mmproj` projector, pulled with `hf-hub` into the clean
//!   app-data layout (`MODEL_REGISTRY §4`).
//!
//! The selection logic (`pick_vulkan_asset`, `pick_gguf_files`) is pure and
//! unit-tested; the network fetches run at app first-run and in the gated manual
//! smoke (they are not exercised by `cargo test`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hf_hub::api::tokio::{ApiBuilder, ApiRepo};
use serde::Deserialize;

use crate::models::{self, ModelLane, ModelTier};

/// GitHub "latest release" endpoint for the upstream llama.cpp project.
const GITHUB_LATEST: &str = "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest";
/// GitHub requires a User-Agent on API requests.
const USER_AGENT: &str = "screensearch-v2c";

#[derive(Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}
#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Picks the Windows x64 Vulkan release asset (`*-bin-win-vulkan-x64.zip`) from a list
/// of asset names. Returns its index, or `None` if the release has no such asset.
pub fn pick_vulkan_asset(names: &[String]) -> Option<usize> {
    names.iter().position(|n| {
        let n = n.to_ascii_lowercase();
        n.ends_with(".zip") && n.contains("win") && n.contains("vulkan") && n.contains("x64")
    })
}

/// From a repo's file list, picks the weights GGUF (preferring a `Q4_K_M` quant,
/// excluding the projector) and — when `needs_mmproj` — the `mmproj*.gguf` projector.
/// Both are chosen deterministically (sorted) so re-resolution is stable.
pub fn pick_gguf_files(
    filenames: &[String],
    needs_mmproj: bool,
) -> (Option<String>, Option<String>) {
    let is_gguf = |n: &str| n.to_ascii_lowercase().ends_with(".gguf");
    let is_mmproj = |n: &str| {
        Path::new(n)
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.to_ascii_lowercase().starts_with("mmproj"))
    };

    let mut weights: Vec<String> = filenames
        .iter()
        .filter(|n| is_gguf(n) && !is_mmproj(n))
        .cloned()
        .collect();
    weights.sort();
    let gguf = weights
        .iter()
        .find(|n| n.to_ascii_lowercase().contains("q4_k_m"))
        .cloned()
        .or_else(|| weights.first().cloned());

    let mmproj = if needs_mmproj {
        let mut mp: Vec<String> = filenames
            .iter()
            .filter(|n| is_gguf(n) && is_mmproj(n))
            .cloned()
            .collect();
        mp.sort();
        mp.into_iter().next()
    } else {
        None
    };

    (gguf, mmproj)
}

/// Ensures `llama-server.exe` exists under `sidecar_dir`, downloading + extracting the
/// prebuilt Vulkan release if absent. Returns the path to the binary. Idempotent.
pub async fn ensure_binary(sidecar_dir: &Path) -> Result<PathBuf> {
    let install = sidecar_dir.join("llama");
    if let Some(found) = find_file(&install, "llama-server.exe") {
        return Ok(found);
    }
    std::fs::create_dir_all(&install).context("create sidecar install dir")?;

    let (url, name) = resolve_binary_url().await?;
    tracing::info!(asset = %name, "downloading llama.cpp Vulkan release");
    let bytes = http_get_bytes(&url).await?;
    extract_zip(&bytes, &install).context("extract llama release zip")?;

    find_file(&install, "llama-server.exe")
        .with_context(|| format!("llama-server.exe not found in release asset {name}"))
}

/// Ensures the `(lane, tier)` model files are present under the app-data models dir,
/// downloading the weights (+ same-repo projector for vision) if missing. Idempotent —
/// a file already present is left as-is.
pub async fn ensure_model(models_root: &Path, lane: ModelLane, tier: ModelTier) -> Result<()> {
    let dir = models::local_dir(models_root, lane, tier);
    std::fs::create_dir_all(&dir).context("create model dir")?;

    let repo = models::repo_for(lane, tier);
    let api = ApiBuilder::new()
        .with_progress(false)
        .with_cache_dir(models_root.join(".hf-cache"))
        .build()
        .context("build hf-hub api")?;
    let repo_api = api.model(repo.repo_id.to_string());

    let info = repo_api
        .info()
        .await
        .with_context(|| format!("list files in {}", repo.repo_id))?;
    let names: Vec<String> = info.siblings.into_iter().map(|s| s.rfilename).collect();
    let (gguf, mmproj) = pick_gguf_files(&names, repo.needs_mmproj);

    let gguf = gguf.with_context(|| format!("no GGUF weights found in {}", repo.repo_id))?;
    download_into(&repo_api, &gguf, &dir).await?;

    if repo.needs_mmproj {
        let mmproj =
            mmproj.with_context(|| format!("no mmproj projector found in {}", repo.repo_id))?;
        download_into(&repo_api, &mmproj, &dir).await?;
    }
    Ok(())
}

/// Downloads one repo file (via the HF cache) and copies it into our clean layout so
/// [`crate::models::resolve_spec`] can find it by scanning `dir`. Skips if present.
async fn download_into(repo: &ApiRepo, filename: &str, dir: &Path) -> Result<()> {
    let base = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let dest = dir.join(base);
    if dest.exists() {
        return Ok(());
    }
    let cached = repo
        .get(filename)
        .await
        .with_context(|| format!("download {filename}"))?;
    std::fs::copy(&cached, &dest)
        .with_context(|| format!("copy {filename} into {}", dir.display()))?;
    Ok(())
}

async fn resolve_binary_url() -> Result<(String, String)> {
    if let Ok(url) = std::env::var("SSV2C_LLAMA_RELEASE_URL") {
        let name = url.rsplit('/').next().unwrap_or("llama.zip").to_string();
        return Ok((url, name));
    }
    let client = reqwest::Client::new();
    let resp = client
        .get(GITHUB_LATEST)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("query llama.cpp latest release")?
        .error_for_status()
        .context("llama.cpp release query returned an error status")?;
    let release: GithubRelease = resp.json().await.context("decode release json")?;
    let names: Vec<String> = release.assets.iter().map(|a| a.name.clone()).collect();
    let idx = pick_vulkan_asset(&names)
        .context("no win-vulkan-x64 asset in the latest llama.cpp release")?;
    let asset = &release.assets[idx];
    Ok((asset.browser_download_url.clone(), asset.name.clone()))
}

async fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .with_context(|| format!("download {url}"))?
        .error_for_status()
        .context("download returned an error status")?;
    Ok(resp.bytes().await.context("read download body")?.to_vec())
}

/// Extracts a zip archive into `dest`, preserving its internal structure.
fn extract_zip(bytes: &[u8], dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("open zip archive")?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            continue; // skip unsafe / absolute paths
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut file)?;
        }
    }
    Ok(())
}

/// Recursively searches `dir` for a file named `name` (case-insensitive).
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(found) = find_file(&p, name) {
                return Some(found);
            }
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
        {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_win_vulkan_x64_asset() {
        let names = vec![
            "llama-b6500-bin-win-cpu-x64.zip".to_string(),
            "llama-b6500-bin-win-vulkan-x64.zip".to_string(),
            "llama-b6500-bin-ubuntu-vulkan-x64.zip".to_string(),
            "cudart-llama-bin-win-cuda-12.4-x64.zip".to_string(),
        ];
        let idx = pick_vulkan_asset(&names).expect("vulkan asset");
        assert_eq!(names[idx], "llama-b6500-bin-win-vulkan-x64.zip");
    }

    #[test]
    fn no_vulkan_asset_returns_none() {
        let names = vec!["llama-b6500-bin-win-cpu-x64.zip".to_string()];
        assert!(pick_vulkan_asset(&names).is_none());
    }

    #[test]
    fn picks_q4_k_m_weights_and_mmproj_for_vision() {
        let files = vec![
            "README.md".to_string(),
            "Qwen3-VL-4B-Instruct-Q8_0.gguf".to_string(),
            "Qwen3-VL-4B-Instruct-Q4_K_M.gguf".to_string(),
            "mmproj-Qwen3-VL-4B-Instruct-f16.gguf".to_string(),
        ];
        let (gguf, mmproj) = pick_gguf_files(&files, true);
        assert_eq!(gguf.as_deref(), Some("Qwen3-VL-4B-Instruct-Q4_K_M.gguf"));
        assert_eq!(
            mmproj.as_deref(),
            Some("mmproj-Qwen3-VL-4B-Instruct-f16.gguf")
        );
    }

    #[test]
    fn answer_repo_needs_no_mmproj() {
        let files = vec!["Ministral-3-3B-Reasoning-Q4_K_M.gguf".to_string()];
        let (gguf, mmproj) = pick_gguf_files(&files, false);
        assert_eq!(
            gguf.as_deref(),
            Some("Ministral-3-3B-Reasoning-Q4_K_M.gguf")
        );
        assert!(mmproj.is_none());
    }

    #[test]
    fn falls_back_to_first_gguf_when_no_q4_k_m() {
        let files = vec![
            "model-Q5_K_M.gguf".to_string(),
            "model-Q8_0.gguf".to_string(),
        ];
        let (gguf, _) = pick_gguf_files(&files, false);
        // Sorted, first is the Q5 — a deterministic fallback.
        assert_eq!(gguf.as_deref(), Some("model-Q5_K_M.gguf"));
    }
}
