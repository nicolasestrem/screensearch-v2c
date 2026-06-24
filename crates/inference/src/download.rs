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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use hf_hub::api::tokio::{ApiBuilder, ApiError, ApiRepo, Progress};
use hf_hub::{Cache, Repo, RepoType};
use serde::Deserialize;
use tokio::sync::{broadcast, Mutex as AsyncMutex};
use traits::{ModelDownloadPhase, ModelDownloadStatus};

use crate::models::{self, ModelLane, ModelTier};

/// How often the downloader samples real downloaded-byte progress (for the UI) and
/// re-checks for a stall while a fetch runs.
const PROGRESS_POLL: Duration = Duration::from_millis(750);

/// If no new bytes land for this long the download is treated as stalled and aborted.
/// hf-hub 0.4.3 builds its own `reqwest` client with **no** socket timeout and we run it
/// with no retries, so a dead CDN connection would otherwise block `bytes_stream().next()`
/// forever (the exact "download hanging" the partial 8B fetch hit). The partial is left on
/// disk, so the next attempt resumes where this one stopped. The local clean-layout copy of a
/// multi-GB blob (during which no new bytes are streamed) is *not* counted against this
/// window — the watchdog pauses while the `copying` flag is set (see `run_download`).
const STALL_TIMEOUT: Duration = Duration::from_secs(180);

/// Timeout for small JSON API probes (the HF tree-API size lookup, the GitHub releases
/// query). These run *before* any stall watchdog, so without a per-request timeout a hung
/// or slow endpoint would block the whole flow from starting (the watchdog can't rescue
/// it yet). Short and bounded — the responses are tiny and the work proceeds either way.
const HTTP_API_TIMEOUT: Duration = Duration::from_secs(15);

/// Connect timeout for the (large) binary download. A total timeout would wrongly abort a
/// legitimately slow multi-MB transfer, so we only guard the *connect* phase — enough to
/// fail fast on a dead host without capping a download that is actually making progress.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Per-read-chunk timeout for the binary download body. Unlike a wall-clock `timeout`, this
/// fails a connection that goes *silent* mid-transfer (CDN accepts the socket then stalls)
/// without aborting a slow-but-progressing download — the binary path has no separate stall
/// watchdog, so `resp.bytes().await` would otherwise hang `init_inference` forever.
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// hf-hub guards each cached blob with a per-file **advisory** lock and gives up after a few
/// seconds with [`ApiError::LockAcquisition`] when another *live* downloader holds it. The app
/// enforces single-instance (so this is rare) and serializes per lane in-process, but a brief
/// old-instance→new-instance startup overlap could still race a fetch. Rather than surface a
/// hard error — which the vision scheduler then retries in a tight loop (the observed "download
/// storm") — we wait for the holder to release and re-attempt with linear backoff. A lock
/// failure streams no bytes, so retrying never double-counts progress. (A lock left by a *dead*
/// process is released by the OS, so this only ever waits on a genuinely live holder.)
const LOCK_RETRY_BACKOFF: Duration = Duration::from_secs(2);
const LOCK_RETRY_MAX_ATTEMPTS: u32 = 5;

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

/// The base filename (no directory) a repo file takes in our clean layout.
fn clean_base(filename: &str) -> &str {
    Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
}

/// Copies a cached blob into our clean `<models_root>/<lane>/<tier>` layout so
/// [`crate::models::resolve_spec`] can find it by scanning the dir. Atomic: write to a temp
/// file in the same dir, then rename. An interrupted copy must never leave a partial file at
/// `dest` — a later `dest.exists()` skip would otherwise treat a corrupt GGUF as complete
/// and crash the sidecar.
fn place_in_clean_layout(cached: &Path, dir: &Path, base: &str) -> Result<()> {
    let dest = dir.join(base);
    let tmp = dir.join(format!("{base}.partial"));
    std::fs::copy(cached, &tmp).with_context(|| format!("copy {base} into {}", dir.display()))?;
    if let Err(e) = std::fs::rename(&tmp, &dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("finalize {}", dest.display()));
    }
    Ok(())
}

/// [`place_in_clean_layout`] off the async runtime — copying a multi-GB blob would otherwise
/// block a worker thread for seconds (starving the event bridge and other requests).
async fn place_in_clean_layout_async(cached: PathBuf, dir: &Path, base: &str) -> Result<()> {
    let dir = dir.to_path_buf();
    let base = base.to_string();
    tokio::task::spawn_blocking(move || place_in_clean_layout(&cached, &dir, &base))
        .await
        .context("clean-layout copy task failed")?
}

/// Downloads one repo file (via the HF cache) and copies it into our clean layout so
/// [`crate::models::resolve_spec`] can find it by scanning `dir`. Skips if present. No
/// progress reporting — used by the gated smoke tests; the app path uses [`ModelDownloader`].
async fn download_into(repo: &ApiRepo, filename: &str, dir: &Path) -> Result<()> {
    let base = clean_base(filename);
    if dir.join(base).exists() {
        return Ok(());
    }
    let cached = repo
        .get(filename)
        .await
        .with_context(|| format!("download {filename}"))?;
    place_in_clean_layout(&cached, dir, base)
}

async fn resolve_binary_url() -> Result<(String, String)> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_API_TIMEOUT)
        .build()
        .context("build releases http client")?;
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
    let client = reqwest::Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .read_timeout(HTTP_READ_TIMEOUT)
        .build()
        .context("build download http client")?;
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

/// Coordinates model downloads: serializes them per lane (so concurrent enrichment
/// workers never race the same multi-GB fetch — the race that dead-lettered the quality
/// vision jobs) and broadcasts [`ModelDownloadStatus`] so the UI can show progress and
/// surface errors instead of opaque network activity (`03 §6/§7`). Construct one at the
/// composition root; share it with both lane providers and bridge [`Self::subscribe`].
pub struct ModelDownloader {
    models_root: PathBuf,
    events: broadcast::Sender<ModelDownloadStatus>,
    /// Per-lane download mutex. Index by lane: Vision = 0, Answer = 1.
    locks: [AsyncMutex<()>; 2],
}

impl ModelDownloader {
    pub fn new(models_root: PathBuf) -> Arc<Self> {
        let (events, _rx) = broadcast::channel(64);
        Arc::new(Self {
            models_root,
            events,
            locks: [AsyncMutex::new(()), AsyncMutex::new(())],
        })
    }

    /// Subscribe to download progress (bridged to the `model_download` UI event).
    pub fn subscribe(&self) -> broadcast::Receiver<ModelDownloadStatus> {
        self.events.subscribe()
    }

    fn lock_for(&self, lane: ModelLane) -> &AsyncMutex<()> {
        match lane {
            ModelLane::Vision => &self.locks[0],
            ModelLane::Answer => &self.locks[1],
        }
    }

    /// Ensures the `(lane, tier)` model files are on disk, downloading them with progress
    /// events if needed. Idempotent and serialized per lane: a second caller for the same
    /// lane waits for the first to finish, then sees the files present (no double fetch).
    pub async fn ensure(&self, lane: ModelLane, tier: ModelTier) -> Result<()> {
        if models::model_files_present(&self.models_root, lane, tier) {
            return Ok(());
        }
        let _guard = self.lock_for(lane).lock().await;
        // Re-check under the lock: another task may have just completed the download.
        if models::model_files_present(&self.models_root, lane, tier) {
            return Ok(());
        }
        self.run_download(lane, tier).await
    }

    /// Downloads the `(lane, tier)` files, reporting **real** downloaded-byte progress and
    /// aborting a stalled fetch instead of hanging forever.
    ///
    /// Progress is driven by hf-hub's [`Progress`] callbacks (true streamed bytes, resume
    /// aware), not the on-disk file size: hf-hub pre-allocates the `.sync.part` to the full
    /// length up front (`set_len`), so a size poll jumps to ~100% immediately and masks both
    /// real progress *and* a stall. A watchdog reads the live byte counter; if it stops
    /// advancing for [`STALL_TIMEOUT`] the download task is aborted and a retryable error is
    /// surfaced — the partial stays on disk, so the next `ensure` resumes from there.
    async fn run_download(&self, lane: ModelLane, tier: ModelTier) -> Result<()> {
        let repo = models::repo_for(lane, tier);
        let model_label = repo
            .repo_id
            .rsplit('/')
            .next()
            .unwrap_or(repo.repo_id)
            .to_string();
        let dir = models::local_dir(&self.models_root, lane, tier);
        std::fs::create_dir_all(&dir).context("create model dir")?;
        let cache_dir = self.models_root.join(".hf-cache");
        let total = total_download_bytes(repo.repo_id, repo.needs_mmproj).await;

        tracing::info!(repo = repo.repo_id, ?total, "model download started");
        self.emit(
            lane,
            &model_label,
            ModelDownloadPhase::Downloading,
            0,
            total,
            None,
        );

        // Fetch on a child task so the watchdog can abort it on a stall. `copying` flags the
        // local clean-layout copy phases (publishing a cached/just-downloaded blob), which are
        // disk I/O — not network — and can exceed the stall timeout on a slow disk for a
        // multi-GB file. The watchdog pauses its stall counter while it is set.
        let downloaded = Arc::new(AtomicU64::new(0));
        let copying = Arc::new(AtomicBool::new(false));
        let task = {
            let cache_dir = cache_dir.clone();
            let dir = dir.clone();
            let repo_id = repo.repo_id.to_string();
            let needs_mmproj = repo.needs_mmproj;
            let downloaded = downloaded.clone();
            let copying = copying.clone();
            tokio::spawn(async move {
                download_repo_files(
                    &cache_dir,
                    &repo_id,
                    needs_mmproj,
                    &dir,
                    downloaded,
                    copying,
                )
                .await
            })
        };

        let limit = stall_limit(STALL_TIMEOUT, PROGRESS_POLL);
        let mut last_seen = 0u64;
        let mut stale = 0u32;
        loop {
            tokio::time::sleep(PROGRESS_POLL).await;
            let d = downloaded.load(Ordering::Relaxed);
            if task.is_finished() {
                let joined = task.await;
                return self.finish_download(lane, &model_label, total, d, joined);
            }
            self.emit(
                lane,
                &model_label,
                ModelDownloadPhase::Downloading,
                d,
                total,
                None,
            );
            if copying.load(Ordering::Relaxed) {
                // A local clean-layout copy is in progress — disk I/O, not a network
                // transfer. Don't let the network stall watchdog fire on it (a multi-GB copy
                // on a slow disk can outlast the timeout). Hold the counter steady.
                last_seen = d;
                stale = 0;
                continue;
            }
            (last_seen, stale) = stall_step(d, last_seen, stale);
            if stale >= limit {
                task.abort();
                let msg = "download stalled — no data received; click Load to resume".to_string();
                tracing::warn!(
                    repo = repo.repo_id,
                    downloaded = d,
                    "model download stalled; aborting"
                );
                self.emit(
                    lane,
                    &model_label,
                    ModelDownloadPhase::Failed,
                    d,
                    total,
                    Some(msg.clone()),
                );
                return Err(anyhow!(msg));
            }
        }
    }

    /// Emits the terminal download status (Done / Failed) and maps the join result to a
    /// `Result`. `d` is the last real byte count the watchdog observed.
    fn finish_download(
        &self,
        lane: ModelLane,
        model_label: &str,
        total: Option<u64>,
        d: u64,
        joined: Result<Result<()>, tokio::task::JoinError>,
    ) -> Result<()> {
        match joined {
            Ok(Ok(())) => {
                tracing::info!(model = model_label, "model download finished");
                self.emit(
                    lane,
                    model_label,
                    ModelDownloadPhase::Done,
                    total.unwrap_or(d),
                    total,
                    None,
                );
                Ok(())
            }
            Ok(Err(e)) => {
                tracing::warn!(model = model_label, error = %e, "model download failed");
                self.emit(
                    lane,
                    model_label,
                    ModelDownloadPhase::Failed,
                    d,
                    total,
                    Some(e.to_string()),
                );
                Err(e)
            }
            Err(join_err) => {
                let e = anyhow!("download task failed: {join_err}");
                tracing::warn!(model = model_label, error = %e, "model download task error");
                self.emit(
                    lane,
                    model_label,
                    ModelDownloadPhase::Failed,
                    d,
                    total,
                    Some(e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn emit(
        &self,
        lane: ModelLane,
        model: &str,
        phase: ModelDownloadPhase,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        error: Option<String>,
    ) {
        let _ = self.events.send(ModelDownloadStatus {
            lane,
            model: Some(model.to_string()),
            phase,
            downloaded_bytes,
            total_bytes,
            error,
        });
    }
}

/// hf-hub [`Progress`] sink that accumulates real streamed bytes into a shared counter the
/// download watchdog reads. On resume hf-hub reports the already-present prefix via one
/// `update(start)` call, so the count is correct across restarts.
#[derive(Clone)]
struct ByteCounter {
    downloaded: Arc<AtomicU64>,
}

impl Progress for ByteCounter {
    async fn init(&mut self, _size: usize, _filename: &str) {}
    async fn update(&mut self, size: usize) {
        self.downloaded.fetch_add(size as u64, Ordering::Relaxed);
    }
    async fn finish(&mut self) {}
}

// TODO(0.1.1, perf): hf-hub fetches single-stream (~20 MB/s, HF CDN throttles one connection).
// Replace with a parallel chunked downloader (N HTTP Range requests -> one pre-allocated file) to
// saturate bandwidth — tracked in specs/07_KNOWN_GAPS.md ("parallel model download").
/// Resolves the repo's GGUF (+ same-repo mmproj for vision) and fetches each into the clean
/// layout, accumulating real downloaded bytes into `downloaded` for the progress watchdog.
async fn download_repo_files(
    cache_dir: &Path,
    repo_id: &str,
    needs_mmproj: bool,
    dir: &Path,
    downloaded: Arc<AtomicU64>,
    copying: Arc<AtomicBool>,
) -> Result<()> {
    let api = ApiBuilder::new()
        .with_progress(false)
        .with_cache_dir(cache_dir.to_path_buf())
        .build()
        .context("build hf-hub api")?;
    let repo_api = api.model(repo_id.to_string());

    let info = repo_api
        .info()
        .await
        .with_context(|| format!("list files in {repo_id}"))?;
    let names: Vec<String> = info.siblings.into_iter().map(|s| s.rfilename).collect();
    let (gguf, mmproj) = pick_gguf_files(&names, needs_mmproj);

    let gguf = gguf.with_context(|| format!("no GGUF weights found in {repo_id}"))?;
    fetch_one(
        &repo_api,
        cache_dir,
        repo_id,
        &gguf,
        dir,
        &downloaded,
        &copying,
    )
    .await?;

    if needs_mmproj {
        let mmproj = mmproj.with_context(|| format!("no mmproj projector found in {repo_id}"))?;
        fetch_one(
            &repo_api,
            cache_dir,
            repo_id,
            &mmproj,
            dir,
            &downloaded,
            &copying,
        )
        .await?;
    }
    Ok(())
}

/// Fetches one repo file into the clean layout with real-byte progress. Checks in order:
/// already in the clean layout → already a finalized blob in the HF cache (no network) →
/// download (resuming any `.sync.part`). `download_with_progress` advances `downloaded` via
/// hf-hub's [`Progress`] callbacks; the skip paths add the on-disk size manually so the bar
/// reflects bytes that were already present.
async fn fetch_one(
    repo_api: &ApiRepo,
    cache_dir: &Path,
    repo_id: &str,
    filename: &str,
    dir: &Path,
    downloaded: &Arc<AtomicU64>,
    copying: &Arc<AtomicBool>,
) -> Result<()> {
    let base = clean_base(filename);
    let dest = dir.join(base);
    if dest.exists() {
        if let Ok(meta) = std::fs::metadata(&dest) {
            downloaded.fetch_add(meta.len(), Ordering::Relaxed);
        }
        return Ok(());
    }

    // A finalized blob already in the cache (a prior run downloaded it but never copied it
    // into the clean layout): copy it — no network round-trip, no re-download. The copy is
    // disk I/O, so flag it so the network stall watchdog doesn't fire on a slow multi-GB copy.
    let cache = Cache::new(cache_dir.to_path_buf());
    if let Some(cached) = cache
        .repo(Repo::new(repo_id.to_string(), RepoType::Model))
        .get(filename)
    {
        let size = std::fs::metadata(&cached).map(|m| m.len()).unwrap_or(0);
        copying.store(true, Ordering::Relaxed);
        let placed = place_in_clean_layout_async(cached, dir, base).await;
        copying.store(false, Ordering::Relaxed);
        placed?;
        downloaded.fetch_add(size, Ordering::Relaxed);
        return Ok(());
    }

    let cached = download_with_lock_retry(repo_api, filename, downloaded)
        .await
        .with_context(|| format!("download {filename}"))?;
    // The streamed bytes are already counted; the publish copy is local I/O, so flag it to
    // pause the stall watchdog rather than re-counting (which would push the bar past 100%).
    copying.store(true, Ordering::Relaxed);
    let placed = place_in_clean_layout_async(cached, dir, base).await;
    copying.store(false, Ordering::Relaxed);
    placed
}

/// Downloads one repo file via hf-hub, retrying with linear backoff when another *live*
/// downloader holds the per-blob advisory lock ([`ApiError::LockAcquisition`]) — the contention
/// that surfaced as the ~5 s "download failed" + retry storm when two app instances raced the
/// same model. Progress accrues into `downloaded`; a lock failure streams no bytes, so a retry
/// never double-counts. Non-lock errors propagate immediately.
async fn download_with_lock_retry(
    repo_api: &ApiRepo,
    filename: &str,
    downloaded: &Arc<AtomicU64>,
) -> std::result::Result<PathBuf, ApiError> {
    let mut attempt: u32 = 0;
    loop {
        let progress = ByteCounter {
            downloaded: downloaded.clone(),
        };
        match repo_api.download_with_progress(filename, progress).await {
            Ok(path) => return Ok(path),
            Err(e) => {
                attempt += 1;
                if matches!(e, ApiError::LockAcquisition(_)) && attempt < LOCK_RETRY_MAX_ATTEMPTS {
                    let backoff = LOCK_RETRY_BACKOFF * attempt;
                    tracing::warn!(
                        filename,
                        attempt,
                        backoff_secs = backoff.as_secs(),
                        "model file lock held by another download; backing off and retrying"
                    );
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                return Err(e);
            }
        }
    }
}

/// Diagnostic entry point for `examples/repro_8b.rs` (not used by the app): download a single
/// repo file into `cache_dir` with the same lock-contention backoff [`fetch_one`] uses, so the
/// fix can be exercised under concurrent contention without driving the whole app.
#[doc(hidden)]
pub async fn download_file_with_lock_retry_for_diagnostics(
    cache_dir: &Path,
    repo_id: &str,
    filename: &str,
) -> Result<PathBuf> {
    let api = ApiBuilder::new()
        .with_progress(false)
        .with_cache_dir(cache_dir.to_path_buf())
        .build()
        .context("build hf-hub api")?;
    let repo_api = api.model(repo_id.to_string());
    let downloaded = Arc::new(AtomicU64::new(0));
    download_with_lock_retry(&repo_api, filename, &downloaded)
        .await
        .with_context(|| format!("download {filename}"))
}

/// Number of `poll`-interval watchdog ticks with no new bytes that constitutes a stall
/// (`timeout / poll`, never zero).
fn stall_limit(timeout: Duration, poll: Duration) -> u32 {
    let poll_ms = poll.as_millis().max(1);
    (timeout.as_millis() / poll_ms).max(1) as u32
}

/// Advances the stall counter for one watchdog tick: resets to 0 (and raises the high-water
/// mark) when bytes grew, otherwise increments. Returns the new `(last_seen, stale)` pair.
fn stall_step(downloaded: u64, last_seen: u64, stale: u32) -> (u64, u32) {
    if downloaded > last_seen {
        (downloaded, 0)
    } else {
        (last_seen, stale.saturating_add(1))
    }
}

/// Best-effort total download size for a repo's selected files, via the HF tree API. Used
/// only to render a percentage — `None` (size unknown) just falls back to a byte counter.
async fn total_download_bytes(repo_id: &str, needs_mmproj: bool) -> Option<u64> {
    #[derive(Deserialize)]
    struct TreeEntry {
        path: String,
        #[serde(default)]
        size: u64,
    }
    let url = format!("https://huggingface.co/api/models/{repo_id}/tree/main");
    let client = reqwest::Client::builder()
        .timeout(HTTP_API_TIMEOUT)
        .build()
        .ok()?;
    let resp = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let entries: Vec<TreeEntry> = resp.json().await.ok()?;
    let names: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let (gguf, mmproj) = pick_gguf_files(&names, needs_mmproj);
    let size_of = |name: &Option<String>| -> u64 {
        name.as_ref()
            .and_then(|n| entries.iter().find(|e| &e.path == n))
            .map(|e| e.size)
            .unwrap_or(0)
    };
    let total = size_of(&gguf) + if needs_mmproj { size_of(&mmproj) } else { 0 };
    (total > 0).then_some(total)
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

    #[tokio::test]
    async fn byte_counter_accumulates_streamed_bytes() {
        // The Progress sink sums every `update` (incl. the resume-prefix `update(start)`).
        let downloaded = Arc::new(AtomicU64::new(0));
        let mut p = ByteCounter {
            downloaded: downloaded.clone(),
        };
        p.init(100, "weights.gguf").await;
        p.update(40).await; // e.g. resumed prefix
        p.update(60).await; // streamed tail
        p.finish().await;
        assert_eq!(downloaded.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn stall_limit_is_timeout_over_poll_and_never_zero() {
        assert_eq!(
            stall_limit(Duration::from_secs(180), Duration::from_millis(750)),
            240
        );
        // Degenerate inputs still yield a usable (>=1) limit rather than an instant abort.
        assert_eq!(
            stall_limit(Duration::from_millis(0), Duration::from_millis(750)),
            1
        );
        assert_eq!(
            stall_limit(Duration::from_secs(5), Duration::from_millis(0)),
            5000
        );
    }

    #[test]
    fn stall_step_resets_on_progress_and_counts_otherwise() {
        // Progress: reset the stale counter, raise the high-water mark.
        assert_eq!(stall_step(50, 10, 3), (50, 0));
        // No progress at the mark: increment, mark unchanged.
        assert_eq!(stall_step(50, 50, 3), (50, 4));
        // A stale read below the mark (shouldn't happen, but must not regress the mark).
        assert_eq!(stall_step(40, 50, 0), (50, 1));
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
