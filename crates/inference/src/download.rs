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

/// GitHub "list releases" endpoint for the upstream llama.cpp project, newest first.
/// We scan the recent page rather than `/releases/latest`: llama.cpp's CI sometimes
/// publishes a release with an incomplete asset set (no `win-vulkan-x64` zip), and a
/// single-`latest` lookup would then fail outright instead of using the prior build.
const GITHUB_RELEASES: &str =
    "https://api.github.com/repos/ggml-org/llama.cpp/releases?per_page=10";
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

/// From a newest-first list of releases, returns the `(download_url, asset_name)` of the
/// Windows x64 Vulkan zip in the most recent release that actually carries one. Some
/// llama.cpp releases ship an incomplete asset set, so the newest release is not always
/// usable — we take the newest that is. Returns `None` if no recent release has the asset.
fn pick_vulkan_from_releases(releases: &[GithubRelease]) -> Option<(String, String)> {
    releases.iter().find_map(|r| {
        let names: Vec<String> = r.assets.iter().map(|a| a.name.clone()).collect();
        let idx = pick_vulkan_asset(&names)?;
        let asset = &r.assets[idx];
        Some((asset.browser_download_url.clone(), asset.name.clone()))
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
/// `SSV2C_LLAMA_RELEASE_URL` deliberately wins over a normal existing install and
/// lands in a URL-specific override directory, so testers can pin a sidecar build
/// without deleting the app-managed release.
pub async fn ensure_binary(sidecar_dir: &Path) -> Result<PathBuf> {
    if let Some((url, name)) = env_binary_override() {
        let install = sidecar_dir
            .join("llama-override")
            .join(url_fingerprint(&url));
        return ensure_binary_from_url(&install, &url, &name).await;
    }

    let install = sidecar_dir.join("llama");
    if let Some(found) = find_file(&install, "llama-server.exe") {
        return Ok(found);
    }

    let (url, name) = resolve_binary_url().await?;
    ensure_binary_from_url(&install, &url, &name).await
}

async fn ensure_binary_from_url(install: &Path, url: &str, name: &str) -> Result<PathBuf> {
    if let Some(found) = find_file(install, "llama-server.exe") {
        return Ok(found);
    }

    tracing::info!(asset = %name, "downloading llama.cpp Vulkan release");
    let bytes = http_get_bytes(url).await?;
    let install = install.to_path_buf();
    let partial = partial_install_dir(&install);
    let install_for_task = install.clone();
    let partial_for_task = partial.clone();
    let partial_for_cleanup = partial.clone();
    let install_result = tokio::task::spawn_blocking(move || {
        if let Some(parent) = install_for_task.parent() {
            std::fs::create_dir_all(parent).context("create sidecar install parent dir")?;
        }
        if partial_for_task.exists() {
            std::fs::remove_dir_all(&partial_for_task).with_context(|| {
                format!(
                    "remove stale partial install {}",
                    partial_for_task.display()
                )
            })?;
        }
        std::fs::create_dir_all(&partial_for_task).context("create sidecar partial install dir")?;
        extract_zip(&bytes, &partial_for_task).context("extract llama release zip")?;
        if find_file(&install_for_task, "llama-server.exe").is_some() {
            std::fs::remove_dir_all(&partial_for_task).with_context(|| {
                format!(
                    "remove redundant partial install {}",
                    partial_for_task.display()
                )
            })?;
            return Ok(());
        }
        if install_for_task.exists() {
            std::fs::remove_dir_all(&install_for_task).with_context(|| {
                format!(
                    "remove incomplete sidecar install dir {}",
                    install_for_task.display()
                )
            })?;
        }
        std::fs::rename(&partial_for_task, &install_for_task).with_context(|| {
            format!(
                "finalize sidecar install {} -> {}",
                partial_for_task.display(),
                install_for_task.display()
            )
        })?;
        Ok(())
    })
    .await
    .context("sidecar binary install task failed")?;
    if let Err(err) = install_result {
        cleanup_partial_install(partial_for_cleanup).await;
        return Err(err);
    }

    find_file(&install, "llama-server.exe")
        .with_context(|| format!("llama-server.exe not found in release asset {name}"))
}

/// Exact installed `llama-server.exe` paths this app owns under the normal and
/// URL-specific override sidecar install roots. Used as startup reap sentinels.
pub fn installed_binary_candidates(sidecar_dir: &Path) -> Vec<PathBuf> {
    let mut binaries = Vec::new();
    collect_files_named(
        &sidecar_dir.join("llama"),
        "llama-server.exe",
        &mut binaries,
    );
    collect_files_named(
        &sidecar_dir.join("llama-override"),
        "llama-server.exe",
        &mut binaries,
    );
    binaries.sort_by_key(|p| p.to_string_lossy().to_ascii_lowercase());
    binaries.dedup_by(|a, b| {
        a.to_string_lossy()
            .as_ref()
            .eq_ignore_ascii_case(b.to_string_lossy().as_ref())
    });
    binaries
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
    // Copy atomically: write to a temp file in the same dir, then rename. An
    // interrupted copy must never leave a partial file at `dest` — the `dest.exists()`
    // skip above would otherwise treat a corrupt GGUF as complete and crash the sidecar.
    let tmp = dir.join(format!("{base}.partial"));
    std::fs::copy(&cached, &tmp)
        .with_context(|| format!("copy {filename} into {}", dir.display()))?;
    if let Err(e) = std::fs::rename(&tmp, &dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("finalize {}", dest.display()));
    }
    Ok(())
}

async fn resolve_binary_url() -> Result<(String, String)> {
    let client = reqwest::Client::new();
    let resp = client
        .get(GITHUB_RELEASES)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("query llama.cpp releases")?
        .error_for_status()
        .context("llama.cpp release query returned an error status")?;
    let releases: Vec<GithubRelease> = resp.json().await.context("decode releases json")?;
    pick_vulkan_from_releases(&releases)
        .context("no win-vulkan-x64 asset in any recent llama.cpp release")
}

fn env_binary_override() -> Option<(String, String)> {
    let url = std::env::var("SSV2C_LLAMA_RELEASE_URL").ok()?;
    let name = url.rsplit('/').next().unwrap_or("llama.zip").to_string();
    Some((url, name))
}

fn url_fingerprint(url: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for b in url.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn partial_install_dir(install: &Path) -> PathBuf {
    let name = install
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("llama");
    install.with_file_name(format!("{name}.partial"))
}

async fn cleanup_partial_install(partial: PathBuf) {
    let _ = tokio::task::spawn_blocking(move || {
        if partial.exists() {
            let _ = std::fs::remove_dir_all(partial);
        }
    })
    .await;
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

fn collect_files_named(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_named(&path, name, out);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::OnceLock;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

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

    fn asset(name: &str, url: &str) -> GithubAsset {
        GithubAsset {
            name: name.to_string(),
            browser_download_url: url.to_string(),
        }
    }

    #[test]
    fn skips_release_with_incomplete_assets() {
        // Mirrors observed llama.cpp behaviour: a freshly-published "latest" release
        // (e.g. b9753) can carry an incomplete asset set with no win-vulkan-x64 zip,
        // while the previous release (b9752) is complete. We must fall back to it.
        let releases = vec![
            GithubRelease {
                assets: vec![asset("llama-b9753-xcframework.zip", "https://x/9753-xcf")],
            },
            GithubRelease {
                assets: vec![
                    asset("llama-b9752-bin-win-cpu-x64.zip", "https://x/9752-cpu"),
                    asset(
                        "llama-b9752-bin-win-vulkan-x64.zip",
                        "https://x/9752-vulkan",
                    ),
                ],
            },
        ];
        let (url, name) =
            pick_vulkan_from_releases(&releases).expect("vulkan from recent releases");
        assert_eq!(name, "llama-b9752-bin-win-vulkan-x64.zip");
        assert_eq!(url, "https://x/9752-vulkan");
    }

    #[test]
    fn prefers_the_newest_release_that_has_vulkan() {
        let releases = vec![
            GithubRelease {
                assets: vec![asset("llama-b2-bin-win-vulkan-x64.zip", "https://x/b2")],
            },
            GithubRelease {
                assets: vec![asset("llama-b1-bin-win-vulkan-x64.zip", "https://x/b1")],
            },
        ];
        let (url, _name) = pick_vulkan_from_releases(&releases).expect("newest vulkan");
        assert_eq!(url, "https://x/b2");
    }

    #[test]
    fn no_vulkan_in_any_release_returns_none() {
        let releases = vec![GithubRelease {
            assets: vec![asset("llama-b1-bin-win-cpu-x64.zip", "https://x/cpu")],
        }];
        assert!(pick_vulkan_from_releases(&releases).is_none());
    }

    #[tokio::test]
    async fn env_override_wins_over_existing_install() {
        let _guard = ENV_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/llama.zip"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(zip_with_llama_server(b"override"), "application/zip"),
            )
            .mount(&server)
            .await;

        let sidecar_dir = unique_temp_dir("override");
        let normal_binary = sidecar_dir
            .join("llama")
            .join("existing")
            .join("llama-server.exe");
        std::fs::create_dir_all(normal_binary.parent().unwrap()).unwrap();
        std::fs::write(&normal_binary, b"normal").unwrap();

        let override_url = format!("{}/llama.zip", server.uri());
        std::env::set_var("SSV2C_LLAMA_RELEASE_URL", &override_url);
        let found = ensure_binary(&sidecar_dir)
            .await
            .expect("override binary should resolve");
        std::env::remove_var("SSV2C_LLAMA_RELEASE_URL");

        let override_install = sidecar_dir
            .join("llama-override")
            .join(url_fingerprint(&override_url));
        assert_ne!(
            found, normal_binary,
            "env override must not reuse a normal existing install"
        );
        assert!(
            found.starts_with(&override_install),
            "override binary should live under {} but got {}",
            override_install.display(),
            found.display()
        );
        assert!(
            !partial_install_dir(&override_install).exists(),
            "successful install should not leave a partial directory"
        );
        assert_eq!(std::fs::read(&found).unwrap(), b"override");
        assert_eq!(
            std::fs::read(&normal_binary).unwrap(),
            b"normal",
            "normal install must be preserved"
        );
        let _ = std::fs::remove_dir_all(sidecar_dir);
    }

    #[tokio::test]
    async fn failed_binary_extraction_cleans_partial_install() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bad.zip"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(b"this is not a zip".to_vec(), "application/zip"),
            )
            .mount(&server)
            .await;

        let root = unique_temp_dir("bad-zip");
        let install = root.join("llama");
        let err = ensure_binary_from_url(&install, &format!("{}/bad.zip", server.uri()), "bad.zip")
            .await
            .expect_err("invalid zip should fail");

        assert!(
            err.to_string().contains("extract llama release zip"),
            "unexpected error: {err:?}"
        );
        assert!(
            !partial_install_dir(&install).exists(),
            "failed extraction must clean up the partial install dir"
        );
        assert!(
            !install.exists(),
            "failed extraction must not publish an install dir"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn installed_binary_candidates_include_normal_and_overrides() {
        let sidecar_dir = unique_temp_dir("candidates");
        let normal = sidecar_dir
            .join("llama")
            .join("release")
            .join("llama-server.exe");
        let override_a = sidecar_dir
            .join("llama-override")
            .join("aaaaaaaaaaaaaaaa")
            .join("bin")
            .join("llama-server.exe");
        let unrelated = sidecar_dir.join("other").join("llama-server.exe");
        for path in [&normal, &override_a, &unrelated] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"exe").unwrap();
        }

        let found = installed_binary_candidates(&sidecar_dir);

        assert!(found.contains(&normal), "missing normal install candidate");
        assert!(
            found.contains(&override_a),
            "missing override install candidate"
        );
        assert!(
            !found.contains(&unrelated),
            "candidate scan must stay inside owned install roots"
        );
        let _ = std::fs::remove_dir_all(sidecar_dir);
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

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ssv2c-download-{tag}-{}-{suffix}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn zip_with_llama_server(contents: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(
                "llama-b-test/bin/llama-server.exe",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(contents).unwrap();
        writer.finish().unwrap().into_inner()
    }
}
