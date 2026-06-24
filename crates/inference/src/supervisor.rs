//! The `ModelSupervisor` — owns the single `llama-server` sidecar's whole lifecycle
//! (`03 §6`): Job-Object binding (no orphan), startup reap of a prior run's stray,
//! lazy spawn on first request, `/health` gating, idle eviction, and model switching
//! (stop + restart with a new GGUF; vision adds `--mmproj`). It broadcasts
//! [`SidecarStatus`] transitions for the readiness panel.
//!
//! Concurrency model: one process at a time. [`acquire`](ModelSupervisor::acquire)
//! ensures the requested model is running (spawning or switching as needed) and hands
//! back a [`Lease`] carrying a [`SidecarClient`]; the lease counts the request as
//! in-flight so the idle-evictor never pulls the model out from under a live request.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tokio::sync::{broadcast, Mutex, OwnedSemaphorePermit, Semaphore};
use traits::{FlashAttnSetting, KvCacheType, SidecarState, SidecarStatus};

use crate::client::SidecarClient;
use crate::flags::{FlashAttnKind, SidecarCaps};
use crate::job_object::JobObject;
use crate::models::ModelSpec;
use crate::process;

/// How often the idle-evictor wakes to check the model's idle time.
const EVICT_TICK: Duration = Duration::from_secs(5);
/// How long to wait between `/health` polls while a model loads.
const HEALTH_POLL: Duration = Duration::from_millis(250);
/// Max time to wait for a killed sidecar to exit (so its VRAM is freed) on a switch.
const PROCESS_EXIT_WAIT: Duration = Duration::from_secs(3);
/// How many times to (re)spawn the sidecar before surfacing a startup failure.
const SPAWN_ATTEMPTS: u32 = 3;
/// Large enough to allow practical same-model parallelism while letting a model switch
/// drain every request slot before terminating the old process.
const REQUEST_GATE_PERMITS: u32 = 1024;

/// Static configuration for a [`ModelSupervisor`].
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Path to the resolved `llama-server.exe` (under app-data, per the runtime
    /// download). Used to launch and always included as a reap sentinel.
    pub binary: PathBuf,
    /// Additional exact `llama-server.exe` image paths this app previously installed.
    /// Startup reap checks these alongside [`Self::binary`] so an override toggle does
    /// not leave an app-owned sidecar running from the old normal/override install.
    pub reap_binaries: Vec<PathBuf>,
    /// Where the child's pid is recorded for the startup reap.
    pub pidfile: PathBuf,
    /// Stop the sidecar after this long with no in-flight request (`sidecar.idle_ttl_secs`).
    pub idle_ttl: Duration,
    /// Max time to wait for `/health` after a spawn before declaring failure.
    pub health_timeout: Duration,
    /// Which memory-tuning flags the bundled `llama-server` accepts (probed once at
    /// init). `build_args` consults this so it only emits flags the binary understands —
    /// the binary auto-updates to the latest release, so its flag set is not fixed.
    pub caps: SidecarCaps,
}

/// A currently-running sidecar process and the client bound to it.
struct SidecarProcess {
    child: process::SuspendedChild,
    client: SidecarClient,
    spec: ModelSpec,
}

/// A handle to the running sidecar for the duration of one request. While a lease is
/// alive the request is counted in-flight (eviction is blocked); dropping it records
/// activity so the idle clock restarts from the request's end.
pub struct Lease {
    client: SidecarClient,
    in_flight: Arc<AtomicUsize>,
    last_activity: Arc<StdMutex<Instant>>,
    _permit: RequestPermit,
}

impl Lease {
    /// The client bound to the leased sidecar.
    pub fn client(&self) -> &SidecarClient {
        &self.client
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        if let Ok(mut g) = self.last_activity.lock() {
            *g = Instant::now();
        }
    }
}

/// Serializes sidecar use so a model switch cannot terminate a process while another
/// request is still streaming through it. Normal requests acquire one permit and may
/// run concurrently; any path that stops the child drains all permits first.
#[derive(Clone)]
pub struct RequestGate {
    permits: Arc<Semaphore>,
    capacity: u32,
}

pub struct RequestPermit {
    _permit: OwnedSemaphorePermit,
}

impl RequestGate {
    pub fn new() -> Self {
        Self::with_capacity(REQUEST_GATE_PERMITS)
    }

    fn with_capacity(capacity: u32) -> Self {
        assert!(capacity > 0, "request gate capacity must be non-zero");
        Self {
            permits: Arc::new(Semaphore::new(capacity as usize)),
            capacity,
        }
    }

    pub async fn enter(&self) -> Result<RequestPermit> {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .context("sidecar request gate closed")?;
        Ok(RequestPermit { _permit: permit })
    }

    pub async fn enter_for_model_switch(&self) -> Result<RequestPermit> {
        let permit = self
            .permits
            .clone()
            .acquire_many_owned(self.capacity)
            .await
            .context("sidecar request gate closed")?;
        Ok(RequestPermit { _permit: permit })
    }
}

impl Default for RequestGate {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestPermit {
    fn into_single(mut self) -> Self {
        if self._permit.num_permits() == 1 {
            return self;
        }
        let permit = self
            ._permit
            .split(1)
            .expect("exclusive sidecar permit should contain at least one permit");
        Self { _permit: permit }
    }
}

/// Owns the sidecar lifecycle. Construct via [`ModelSupervisor::new`] inside a Tokio
/// runtime (it spawns the idle-evictor task).
pub struct ModelSupervisor {
    config: SupervisorConfig,
    job: JobObject,
    state: Mutex<Option<SidecarProcess>>,
    in_flight: Arc<AtomicUsize>,
    last_activity: Arc<StdMutex<Instant>>,
    events: broadcast::Sender<SidecarStatus>,
    shutdown: AtomicBool,
    /// When set, the idle-evictor holds off (the kernel's idle vision backfill is draining
    /// the backlog and wants the model kept warm). Cleared when the backlog is empty or
    /// the user resumes, after which normal idle eviction frees the VRAM (`03 §5/§6`).
    backfill_active: AtomicBool,
    /// Set by a manual "Load model" and cleared by "Unload": the user explicitly asked to
    /// keep this model resident, so the idle-TTL must not evict it (that was the
    /// "evicted right after I downloaded it" surprise). Unload or app exit clears it.
    pinned: AtomicBool,
    gate: RequestGate,
}

impl ModelSupervisor {
    /// Creates the supervisor: a `KILL_ON_JOB_CLOSE` job, a startup reap of any stray
    /// `llama-server` this app left behind, and the background idle-evictor. Must run
    /// inside a Tokio runtime.
    pub fn new(config: SupervisorConfig) -> Result<Arc<Self>> {
        let job = JobObject::new().context("create job object")?;

        // Startup reap: kill a stray sidecar from a prior run, identified by pidfile +
        // exact image-path sentinels (never an unrelated process) — `03 §6`.
        let mut reap_binaries = config.reap_binaries.clone();
        reap_binaries.push(config.binary.clone());
        if reap_stray_any(&config.pidfile, &reap_binaries) {
            tracing::warn!("startup reap terminated a stray sidecar from a prior run");
        }

        let (events, _rx) = broadcast::channel(64);
        let me = Arc::new(Self {
            config,
            job,
            state: Mutex::new(None),
            in_flight: Arc::new(AtomicUsize::new(0)),
            last_activity: Arc::new(StdMutex::new(Instant::now())),
            events,
            shutdown: AtomicBool::new(false),
            backfill_active: AtomicBool::new(false),
            pinned: AtomicBool::new(false),
            gate: RequestGate::new(),
        });
        me.clone().spawn_evictor();
        Ok(me)
    }

    /// Subscribe to sidecar lifecycle transitions (forwarded to `sidecar_status`).
    pub fn subscribe(&self) -> broadcast::Receiver<SidecarStatus> {
        self.events.subscribe()
    }

    /// Ensures `spec` is the running model (spawning or switching as needed) and
    /// returns a [`Lease`] to it. The lease keeps the request in-flight; the caller
    /// runs its HTTP request through `lease.client()`.
    pub async fn acquire(&self, spec: ModelSpec) -> Result<Lease> {
        loop {
            if self.needs_exclusive_switch(&spec).await {
                let permit = self.gate.enter_for_model_switch().await?;
                let mut guard = self.state.lock().await;
                if !guard
                    .as_ref()
                    .is_some_and(|p| needs_restart(&p.spec, &spec))
                {
                    drop(guard);
                    drop(permit);
                    continue;
                }
                if let Some(old) = guard.take() {
                    // Wait for the old process to fully exit before spawning the new model
                    // so its GPU memory is released first (avoids a VRAM-allocation race on
                    // model switch). A switch to a *different* model also drops any manual
                    // pin — it belonged to the model being replaced (a manual Load re-pins
                    // after this returns).
                    self.pinned.store(false, Ordering::SeqCst);
                    self.stop_child(old).await;
                }
                let proc = self.spawn_with_retries(&spec).await?;
                *guard = Some(proc);
                let lease = self.lease_from_state(
                    guard.as_ref().expect("sidecar present after switch"),
                    permit.into_single(),
                );
                drop(guard);
                return Ok(lease);
            }

            let permit = self.gate.enter().await?;
            let mut guard = self.state.lock().await;
            match guard.as_ref() {
                Some(running) if needs_restart(&running.spec, &spec) => {
                    drop(guard);
                    drop(permit);
                    continue;
                }
                Some(running) if running_sidecar_healthy(running).await => {
                    let lease = self.lease_from_state(running, permit);
                    drop(guard);
                    return Ok(lease);
                }
                Some(running) => {
                    let crashed_spec = running.spec.clone();
                    drop(guard);
                    drop(permit);

                    let permit = self.gate.enter_for_model_switch().await?;
                    let mut guard = self.state.lock().await;
                    if let Some(running) = guard.as_ref() {
                        if !needs_restart(&running.spec, &spec)
                            && running_sidecar_healthy(running).await
                        {
                            drop(guard);
                            drop(permit);
                            continue;
                        }
                        self.emit(SidecarState::Crashed, Some(&crashed_spec));
                    } else {
                        drop(guard);
                        drop(permit);
                        continue;
                    }
                    if let Some(old) = guard.take() {
                        self.stop_child(old).await;
                    }
                    let proc = self.spawn_with_retries(&spec).await?;
                    *guard = Some(proc);
                    let lease = self.lease_from_state(
                        guard
                            .as_ref()
                            .expect("sidecar present after crash recovery"),
                        permit.into_single(),
                    );
                    drop(guard);
                    return Ok(lease);
                }
                None => {
                    let proc = self.spawn_with_retries(&spec).await?;
                    *guard = Some(proc);
                    let lease = self.lease_from_state(
                        guard.as_ref().expect("sidecar present after initial spawn"),
                        permit,
                    );
                    drop(guard);
                    return Ok(lease);
                }
            }
        }
    }

    async fn needs_exclusive_switch(&self, spec: &ModelSpec) -> bool {
        self.state
            .lock()
            .await
            .as_ref()
            .is_some_and(|p| needs_restart(&p.spec, spec))
    }

    fn lease_from_state(&self, running: &SidecarProcess, permit: RequestPermit) -> Lease {
        let client = running.client.clone();
        // Count the request in-flight **while still holding the state lock**: the
        // evictor re-checks `in_flight` under this same lock before killing, so it can
        // never evict the sidecar we are handing out (closes the drop→fetch_add race).
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut g) = self.last_activity.lock() {
            *g = Instant::now();
        }
        Lease {
            client,
            in_flight: self.in_flight.clone(),
            last_activity: self.last_activity.clone(),
            _permit: permit,
        }
    }

    /// Kills a running sidecar and waits (bounded) for the OS to release it, so its GPU
    /// memory is freed before the next model spawns.
    async fn stop_child(&self, old: SidecarProcess) {
        self.kill_and_confirm("model switch", old.child).await;
    }

    /// Kills `child`, waits (bounded) for the OS to release the process so its VRAM/RAM is
    /// actually freed, removes the pidfile, and logs whether the kill took effect. Every
    /// teardown path (idle eviction, manual unload, model switch, shutdown) routes through
    /// here so a kill that silently fails is visible instead of leaving VRAM occupied
    /// while the UI claims the model is gone (`03 §6`). Polls rather than blocking the
    /// executor on `WaitForSingleObject`.
    async fn kill_and_confirm(&self, reason: &str, child: process::SuspendedChild) {
        let pid = child.pid();
        let killed = child.kill();
        let deadline = Instant::now() + PROCESS_EXIT_WAIT;
        while process::pid_alive(pid) && Instant::now() < deadline {
            tokio::time::sleep(HEALTH_POLL).await;
        }
        let still_alive = process::pid_alive(pid);
        if still_alive {
            // Keep the pidfile: the process outlived the kill, so leave the record on disk
            // for the next launch's `reap_stray_any` to find and terminate. Deleting it here
            // would strand the orphan (and its VRAM) with nothing pointing the reaper at it.
            tracing::warn!(
                reason,
                pid,
                killed,
                "sidecar kill did not terminate the process within the exit wait; VRAM may \
                 still be held"
            );
        } else {
            let _ = std::fs::remove_file(&self.config.pidfile);
            tracing::debug!(reason, pid, killed, "sidecar process terminated");
        }
    }

    /// Spawns the sidecar, retrying a few times. Each attempt allocates a fresh port,
    /// which also hardens against the (rare) race where the chosen ephemeral port is
    /// taken between `free_port` and `llama-server` binding it. `spawn_for` kills the
    /// child on any failure, so a retry never leaks a process.
    async fn spawn_with_retries(&self, spec: &ModelSpec) -> Result<SidecarProcess> {
        let mut last_err = None;
        for attempt in 1..=SPAWN_ATTEMPTS {
            match self.spawn_for(spec).await {
                Ok(proc) => return Ok(proc),
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "sidecar spawn failed; retrying");
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("sidecar spawn failed")))
    }

    /// Stops the sidecar and disables further eviction work (called on app exit).
    pub async fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let mut guard = self.state.lock().await;
        if let Some(p) = guard.take() {
            self.kill_and_confirm("shutdown", p.child).await;
        }
        self.emit(SidecarState::Stopped, None);
    }

    /// Eagerly loads `spec` (spawning or switching as needed) so the next real request is
    /// instant — the manual "Load model" control. The lease is dropped immediately, which
    /// starts the idle clock; the model then stays resident until the idle-TTL or a manual
    /// [`Self::unload`] reclaims it.
    pub async fn preload(&self, spec: ModelSpec) -> Result<()> {
        let _lease = self.acquire(spec).await?;
        // Pin: an explicit Load means "keep this resident" — don't idle-evict it until the
        // user unloads (otherwise the model the user just waited to download/load would be
        // evicted the moment the idle-TTL elapsed).
        self.pinned.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Manually stops the resident sidecar, freeing its VRAM/RAM now — the "Unload" control.
    /// Drains the request gate first so a live request is never torn out, then kills the
    /// process and reports `Evicted` (the next request lazily respawns). No-op when nothing
    /// is resident.
    pub async fn unload(&self) {
        // Clear the manual pin first: an explicit unload overrides "keep resident".
        self.pinned.store(false, Ordering::SeqCst);
        let Ok(_permit) = self.gate.enter_for_model_switch().await else {
            return;
        };
        let mut guard = self.state.lock().await;
        if let Some(p) = guard.take() {
            self.kill_and_confirm("manual unload", p.child).await;
            self.emit(SidecarState::Evicted, Some(&p.spec));
            tracing::info!("sidecar unloaded on request");
        }
    }

    /// Spawns `llama-server` for `spec`, binds it to the job **before** resuming, then
    /// waits for `/health`. On any failure the child is killed so nothing leaks.
    async fn spawn_for(&self, spec: &ModelSpec) -> Result<SidecarProcess> {
        let port = free_port().context("allocate sidecar port")?;
        let args = build_args(spec, port, &self.config.caps);
        self.emit(SidecarState::Starting, Some(spec));

        let child = process::spawn_suspended(&self.config.binary, &args)
            .with_context(|| format!("spawn {}", self.config.binary.display()))?;
        // Assign-before-resume: no window in which the child runs unbound (`03 §6`).
        if let Err(e) = self.job.assign(child.process_handle()) {
            child.kill();
            return Err(e);
        }
        if let Err(e) = child.resume() {
            child.kill();
            return Err(e);
        }
        let _ = std::fs::write(&self.config.pidfile, child.pid().to_string());

        let base = format!("http://127.0.0.1:{port}");
        let client = SidecarClient::new(base);

        let deadline = Instant::now() + self.config.health_timeout;
        loop {
            if !process::pid_alive(child.pid()) {
                self.emit(SidecarState::Crashed, Some(spec));
                bail!("llama-server exited during startup");
            }
            if client.health().await {
                break;
            }
            if Instant::now() >= deadline {
                child.kill();
                self.emit(SidecarState::Crashed, Some(spec));
                bail!(
                    "llama-server did not become healthy within {:?}",
                    self.config.health_timeout
                );
            }
            tokio::time::sleep(HEALTH_POLL).await;
        }

        self.emit(SidecarState::Ready, Some(spec));
        Ok(SidecarProcess {
            child,
            client,
            spec: spec.clone(),
        })
    }

    fn spawn_evictor(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(EVICT_TICK).await;
                if self.shutdown.load(Ordering::SeqCst) {
                    break;
                }
                self.maybe_evict().await;
            }
        });
    }

    /// Stops the sidecar if it has been idle past the TTL, nothing is in flight, and the
    /// kernel's idle backfill is not asking us to stay warm.
    async fn maybe_evict(&self) {
        let elapsed = self
            .last_activity
            .lock()
            .map(|g| g.elapsed())
            .unwrap_or_default();
        if !should_evict(
            self.in_flight.load(Ordering::SeqCst),
            elapsed,
            self.config.idle_ttl,
            self.backfill_active.load(Ordering::SeqCst) || self.pinned.load(Ordering::SeqCst),
        ) {
            return;
        }
        let mut guard = self.state.lock().await;
        // Re-check in-flight under the lock — a request may have arrived between checks.
        // (Backfill is a coarse keep-warm hint, so it's only checked above, not re-locked.)
        if self.in_flight.load(Ordering::SeqCst) > 0 {
            return;
        }
        if let Some(p) = guard.take() {
            self.kill_and_confirm("idle TTL", p.child).await;
            self.emit(SidecarState::Evicted, Some(&p.spec));
            tracing::info!("sidecar evicted after idle TTL");
        }
    }

    /// Broadcasts a lifecycle transition. `spec` (the model this transition concerns)
    /// supplies both the human label and the lane, so the readiness panel can show *which*
    /// model is — or was last — resident. `None` only for the no-model-yet `Stopped`.
    fn emit(&self, state: SidecarState, spec: Option<&ModelSpec>) {
        let _ = self.events.send(SidecarStatus {
            state,
            model: spec.map(model_label),
            lane: spec.map(|s| s.lane),
        });
    }
}

/// Whether switching from `running` to `requested` requires a sidecar restart (a
/// different GGUF or projector). Same model → reuse the running process.
pub fn needs_restart(running: &ModelSpec, requested: &ModelSpec) -> bool {
    running.gguf_path != requested.gguf_path
        || running.mmproj_path != requested.mmproj_path
        || running.ngl != requested.ngl
        || running.device != requested.device
        || running.ctx_size != requested.ctx_size
        || running.kv_cache_type != requested.kv_cache_type
        || running.flash_attn != requested.flash_attn
}

/// Pure idle predicate (extracted for testing): idle once `elapsed >= ttl`.
pub fn idle_expired(elapsed: Duration, ttl: Duration) -> bool {
    elapsed >= ttl
}

/// Pure eviction predicate (extracted for testing): evict only when nothing is in flight,
/// the model has been idle past the TTL, **and** the kernel's idle backfill is not asking
/// us to stay warm. Keeping the model loaded during a backfill drain is the whole point of
/// the keep-warm flag (`03 §5/§6`).
pub fn should_evict(
    in_flight: usize,
    elapsed: Duration,
    ttl: Duration,
    backfill_active: bool,
) -> bool {
    in_flight == 0 && !backfill_active && idle_expired(elapsed, ttl)
}

impl traits::BackfillControl for ModelSupervisor {
    fn set_backfill_active(&self, active: bool) {
        self.backfill_active.store(active, Ordering::SeqCst);
    }
}

/// A running sidecar can be reused only when both the OS process and the HTTP health
/// endpoint are alive. Anything else is treated as a crash/hang and respawned.
pub fn can_reuse_running_sidecar(process_alive: bool, health_ok: bool) -> bool {
    process_alive && health_ok
}

async fn running_sidecar_healthy(running: &SidecarProcess) -> bool {
    let process_alive = process::pid_alive(running.child.pid());
    let health_ok = if process_alive {
        running.client.health().await
    } else {
        false
    };
    can_reuse_running_sidecar(process_alive, health_ok)
}

/// Reaps a stray sidecar a prior run left behind: reads `pidfile`, and only if that
/// pid is alive **and** its image path is our installed `expected_exe` does it
/// terminate the process (`03 §6` — never kill an unrelated process that recycled the
/// pid). Returns whether a process was killed. Stale/foreign pidfiles are cleaned up.
pub fn reap_stray(pidfile: &Path, expected_exe: &Path) -> bool {
    reap_stray_any(pidfile, &[expected_exe.to_path_buf()])
}

/// Like [`reap_stray`], but accepts every exact sidecar executable path the app owns
/// (the selected binary plus prior normal/override installs).
pub fn reap_stray_any(pidfile: &Path, expected_exes: &[PathBuf]) -> bool {
    let Ok(content) = std::fs::read_to_string(pidfile) else {
        return false;
    };
    let Ok(pid) = content.trim().parse::<u32>() else {
        let _ = std::fs::remove_file(pidfile);
        return false;
    };
    if !process::pid_alive(pid) {
        let _ = std::fs::remove_file(pidfile);
        return false;
    }
    let is_ours = process::image_path(pid)
        .is_some_and(|p| expected_exes.iter().any(|expected| paths_eq(&p, expected)));
    if is_ours {
        let killed = process::terminate(pid);
        let _ = std::fs::remove_file(pidfile);
        killed
    } else {
        // Pid recycled to an unrelated process — leave it and the pidfile alone.
        false
    }
}

/// Case-insensitive path comparison (Windows paths are case-insensitive).
fn paths_eq(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// The launch arguments for `llama-server` (`MODEL_REGISTRY §4`): model + host/port +
/// GPU offload, plus the same-repo projector on the vision lane, plus the memory-tuning
/// flags (`--ctx-size`, `--flash-attn`, `--cache-type-k/-v`) — but only those `caps`
/// reports the bundled binary actually accepts, so a future auto-updated build that
/// renames or drops a flag degrades gracefully instead of failing to spawn.
fn build_args(spec: &ModelSpec, port: u16, caps: &SidecarCaps) -> Vec<String> {
    let mut args = vec![
        "--model".to_string(),
        spec.gguf_path.to_string_lossy().into_owned(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        port.to_string(),
        "-ngl".to_string(),
        spec.ngl.to_string(),
    ];
    if let Some(mmproj) = &spec.mmproj_path {
        args.push("--mmproj".to_string());
        args.push(mmproj.to_string_lossy().into_owned());
    }
    if let Some(device) = &spec.device {
        args.push("--device".to_string());
        args.push(device.clone());
    }
    // Pin the context window (the dominant VRAM lever). `spec.ctx_size` is already the
    // resolved value (the auto sentinel was substituted in `resolve_spec`).
    if caps.ctx_size {
        args.push("--ctx-size".to_string());
        args.push(spec.ctx_size.to_string());
    }
    let flash = push_flash_attn(&mut args, caps.flash_attn_kind, spec.flash_attn);
    push_kv_cache(&mut args, caps, spec.kv_cache_type, flash);
    args
}

/// Whether flash attention ends up active and, when it doesn't, *why* — so a quantized-KV
/// downgrade can be diagnosed accurately (a binary limitation vs. the user's own choice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashState {
    /// Flash attention will be on — quantized KV is safe to emit.
    Active,
    /// The binary has no `--flash-attn` flag, so it can't be enabled.
    BinaryUnsupported,
    /// The binary supports it, but the user set `FlashAttnSetting::Off`.
    UserDisabled,
}

/// Appends the `--flash-attn` flag in whatever spelling the binary accepts and reports the
/// resulting [`FlashState`] (which gates KV-cache quantization).
///
/// On a value-taking binary the three settings map to distinct args: `Auto` emits
/// `--flash-attn auto` (defer to llama.cpp's own readiness check), `On` emits
/// `--flash-attn on` (force it), and `Off` emits `--flash-attn off`. `auto` resolves to
/// on for every build that advertises the flag, so it still counts as active for the
/// purpose of unlocking quantized KV. A legacy bare-switch binary has no `auto` spelling,
/// so `Auto`/`On` both append the bare flag. When the binary lacks the flag entirely an
/// explicit `On` is warned about (the user asked for something unavailable) rather than
/// dropped silently.
fn push_flash_attn(
    args: &mut Vec<String>,
    kind: FlashAttnKind,
    setting: FlashAttnSetting,
) -> FlashState {
    match (kind, setting) {
        (FlashAttnKind::Unsupported, FlashAttnSetting::On) => {
            tracing::warn!(
                "flash attention was explicitly enabled in settings, but this llama-server \
                 build does not support --flash-attn; the setting is ignored"
            );
            FlashState::BinaryUnsupported
        }
        (FlashAttnKind::Unsupported, _) => FlashState::BinaryUnsupported,
        (FlashAttnKind::BoolFlag, FlashAttnSetting::Off) => FlashState::UserDisabled,
        (FlashAttnKind::BoolFlag, _) => {
            args.push("--flash-attn".to_string());
            FlashState::Active
        }
        (FlashAttnKind::EnumOnOffAuto, FlashAttnSetting::Off) => {
            args.push("--flash-attn".to_string());
            args.push("off".to_string());
            FlashState::UserDisabled
        }
        (FlashAttnKind::EnumOnOffAuto, FlashAttnSetting::Auto) => {
            args.push("--flash-attn".to_string());
            args.push("auto".to_string());
            FlashState::Active
        }
        (FlashAttnKind::EnumOnOffAuto, FlashAttnSetting::On) => {
            args.push("--flash-attn".to_string());
            args.push("on".to_string());
            FlashState::Active
        }
    }
}

/// Appends `--cache-type-k`/`--cache-type-v` only for a quantized type **and** only when
/// flash attention is active (quantized KV requires it). `f16` (the llama.cpp default) is
/// left implicit, and a quantized request without flash attention degrades to that
/// default with a warning — phrased to match *why* flash is off — rather than producing an
/// arg list the binary would reject.
fn push_kv_cache(args: &mut Vec<String>, caps: &SidecarCaps, kv: KvCacheType, flash: FlashState) {
    if !kv.is_quantized() {
        return; // f16 is the default; nothing to pass.
    }
    match flash {
        FlashState::Active => {}
        FlashState::BinaryUnsupported => {
            tracing::warn!(
                "KV-cache quantization requires flash attention, which this llama-server \
                 build does not support; using the default f16 KV cache"
            );
            return;
        }
        FlashState::UserDisabled => {
            tracing::warn!(
                "KV-cache quantization requires flash attention, which is turned off in \
                 settings; using the default f16 KV cache"
            );
            return;
        }
    }
    if !caps.cache_type_k && !caps.cache_type_v {
        tracing::warn!(
            "KV-cache quantization is configured, but this llama-server build advertises \
             neither --cache-type-k nor --cache-type-v; using the default f16 KV cache"
        );
        return;
    }
    if caps.cache_type_k {
        args.push("--cache-type-k".to_string());
        args.push(kv.as_arg().to_string());
    }
    if caps.cache_type_v {
        args.push("--cache-type-v".to_string());
        args.push(kv.as_arg().to_string());
    }
}

/// A short label (the GGUF filename) for status events.
fn model_label(spec: &ModelSpec) -> String {
    spec.gguf_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("model")
        .to_string()
}

/// Allocates a free ephemeral TCP port on the loopback for the sidecar to bind.
fn free_port() -> Result<u16> {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").context("bind ephemeral loopback port")?;
    Ok(listener.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelLane, ModelTier};

    fn spec(gguf: &str, mmproj: Option<&str>) -> ModelSpec {
        ModelSpec {
            lane: if mmproj.is_some() {
                ModelLane::Vision
            } else {
                ModelLane::Answer
            },
            tier: ModelTier::Default,
            gguf_path: PathBuf::from(gguf),
            mmproj_path: mmproj.map(PathBuf::from),
            ngl: 99,
            device: None,
            ctx_size: 4096,
            kv_cache_type: KvCacheType::Q8_0,
            flash_attn: FlashAttnSetting::Auto,
        }
    }

    /// Caps for a modern binary that advertises every memory-tuning flag with a
    /// value-taking `--flash-attn`.
    fn caps_full() -> SidecarCaps {
        SidecarCaps {
            ctx_size: true,
            cache_type_k: true,
            cache_type_v: true,
            flash_attn_kind: FlashAttnKind::EnumOnOffAuto,
        }
    }

    /// Returns the value following `flag` in an arg list, if present.
    fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.windows(2)
            .find(|w| w[0] == flag)
            .map(|w| w[1].as_str())
    }

    #[test]
    fn restart_only_on_model_change() {
        let a = spec(r"C:\m\a.gguf", None);
        let a2 = spec(r"C:\m\a.gguf", None);
        let b = spec(r"C:\m\b.gguf", None);
        assert!(!needs_restart(&a, &a2), "same gguf → reuse");
        assert!(needs_restart(&a, &b), "different gguf → restart");

        let v1 = spec(r"C:\m\v.gguf", Some(r"C:\m\mmproj1.gguf"));
        let v2 = spec(r"C:\m\v.gguf", Some(r"C:\m\mmproj2.gguf"));
        assert!(needs_restart(&v1, &v2), "different projector → restart");
    }

    #[test]
    fn idle_predicate() {
        assert!(idle_expired(
            Duration::from_secs(200),
            Duration::from_secs(180)
        ));
        assert!(!idle_expired(
            Duration::from_secs(10),
            Duration::from_secs(180)
        ));
    }

    #[test]
    fn evict_predicate_respects_inflight_backfill_and_ttl() {
        let ttl = Duration::from_secs(180);
        let past = Duration::from_secs(200);
        let recent = Duration::from_secs(10);
        // Idle past TTL, nothing in flight, no backfill → evict.
        assert!(should_evict(0, past, ttl, false));
        // A request in flight → never evict.
        assert!(!should_evict(1, past, ttl, false));
        // Backfill draining the backlog → keep warm even when idle past TTL.
        assert!(!should_evict(0, past, ttl, true));
        // Not idle long enough → don't evict.
        assert!(!should_evict(0, recent, ttl, false));
    }

    #[tokio::test]
    async fn request_gate_allows_concurrent_regular_requests() {
        let gate = RequestGate::with_capacity(2);
        let first = gate.enter().await.expect("first request enters");
        let second = gate.enter().await.expect("second request enters");

        let gate_for_third = gate.clone();
        let third_wait = tokio::spawn(async move {
            let _third = gate_for_third
                .enter()
                .await
                .expect("third enters once a request drops");
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !third_wait.is_finished(),
            "only capacity, not one, should limit regular request concurrency"
        );

        drop(first);
        tokio::time::timeout(Duration::from_secs(1), third_wait)
            .await
            .expect("third request should proceed after one slot frees")
            .expect("third request task should not panic");
        drop(second);
    }

    #[tokio::test]
    async fn switch_gate_waits_for_active_request_to_drop() {
        let gate = RequestGate::with_capacity(2);
        let first = gate.enter().await.expect("first request enters");
        let second = gate.enter().await.expect("second request enters");

        let gate_for_switch = gate.clone();
        let switch_wait = tokio::spawn(async move {
            let switch = gate_for_switch
                .enter_for_model_switch()
                .await
                .expect("switch enters once first request drops");
            assert_eq!(
                switch._permit.num_permits(),
                2,
                "switch should drain every request permit"
            );
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !switch_wait.is_finished(),
            "model switch must wait while another request lease is active"
        );

        drop(first);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !switch_wait.is_finished(),
            "model switch must wait for every active request lease"
        );

        drop(second);
        tokio::time::timeout(Duration::from_secs(1), switch_wait)
            .await
            .expect("switch should proceed after active request drops")
            .expect("switch task should not panic");
    }

    #[test]
    fn running_sidecar_is_reused_only_when_process_and_health_are_alive() {
        assert!(can_reuse_running_sidecar(true, true));
        assert!(!can_reuse_running_sidecar(false, true));
        assert!(!can_reuse_running_sidecar(true, false));
        assert!(!can_reuse_running_sidecar(false, false));
    }

    #[test]
    fn restart_on_tuning_change() {
        let base = spec(r"C:\m\a.gguf", None);

        let mut ctx = base.clone();
        ctx.ctx_size = 2048;
        assert!(needs_restart(&base, &ctx), "ctx_size change → restart");

        let mut kv = base.clone();
        kv.kv_cache_type = KvCacheType::F16;
        assert!(needs_restart(&base, &kv), "kv_cache_type change → restart");

        let mut fa = base.clone();
        fa.flash_attn = FlashAttnSetting::Off;
        assert!(needs_restart(&base, &fa), "flash_attn change → restart");
    }

    #[test]
    fn build_args_adds_mmproj_only_for_vision() {
        let answer = build_args(&spec(r"C:\m\a.gguf", None), 8080, &caps_full());
        assert!(!answer.iter().any(|a| a == "--mmproj"));
        assert!(answer
            .windows(2)
            .any(|w| w[0] == "--port" && w[1] == "8080"));

        let vision = build_args(
            &spec(r"C:\m\v.gguf", Some(r"C:\m\mmproj.gguf")),
            8080,
            &caps_full(),
        );
        assert!(vision
            .windows(2)
            .any(|w| w[0] == "--mmproj" && w[1] == r"C:\m\mmproj.gguf"));
    }

    #[test]
    fn build_args_adds_device_when_configured() {
        let mut answer = spec(r"C:\m\a.gguf", None);
        answer.device = Some("Vulkan0".to_string());

        let args = build_args(&answer, 8080, &caps_full());

        assert!(args
            .windows(2)
            .any(|w| w[0] == "--device" && w[1] == "Vulkan0"));
    }

    #[test]
    fn build_args_emits_full_tuning_when_supported() {
        // Modern binary + quantized KV + flash Auto → all four flags. Auto defers to
        // llama.cpp (`--flash-attn auto`), which still counts as active for quantized KV.
        let mut s = spec(r"C:\m\a.gguf", None);
        s.ctx_size = 8192;
        let args = build_args(&s, 8080, &caps_full());
        assert_eq!(value_after(&args, "--ctx-size"), Some("8192"));
        assert_eq!(value_after(&args, "--flash-attn"), Some("auto"));
        assert_eq!(value_after(&args, "--cache-type-k"), Some("q8_0"));
        assert_eq!(value_after(&args, "--cache-type-v"), Some("q8_0"));
    }

    #[test]
    fn build_args_distinguishes_auto_from_on_flash_attn() {
        // On a value-taking binary, `Auto` and `On` must produce distinct args so the
        // setting is observable: Auto → `auto` (defer), On → `on` (force).
        let mut auto = spec(r"C:\m\a.gguf", None);
        auto.flash_attn = FlashAttnSetting::Auto;
        assert_eq!(
            value_after(&build_args(&auto, 8080, &caps_full()), "--flash-attn"),
            Some("auto")
        );

        let mut on = spec(r"C:\m\a.gguf", None);
        on.flash_attn = FlashAttnSetting::On;
        let on_args = build_args(&on, 8080, &caps_full());
        assert_eq!(value_after(&on_args, "--flash-attn"), Some("on"));
        // Forcing flash on still unlocks quantized KV.
        assert_eq!(value_after(&on_args, "--cache-type-v"), Some("q8_0"));
    }

    #[test]
    fn build_args_uses_bare_flash_attn_for_bool_flag() {
        // Legacy binary spells --flash-attn as a bare switch (no value), and flash is
        // active, so quantized KV is still emitted.
        let caps = SidecarCaps {
            flash_attn_kind: FlashAttnKind::BoolFlag,
            ..caps_full()
        };
        let args = build_args(&spec(r"C:\m\a.gguf", None), 8080, &caps);
        assert!(args.iter().any(|a| a == "--flash-attn"));
        // The token after --flash-attn must NOT be a value (it's the next real flag).
        assert_ne!(value_after(&args, "--flash-attn"), Some("on"));
        assert_eq!(value_after(&args, "--cache-type-k"), Some("q8_0"));
    }

    #[test]
    fn build_args_omits_kv_quant_without_flash() {
        // No flash support → quantized KV would be rejected, so it is dropped (f16
        // default), and no --flash-attn is emitted. Context is still pinned.
        let caps = SidecarCaps {
            flash_attn_kind: FlashAttnKind::Unsupported,
            ..caps_full()
        };
        let args = build_args(&spec(r"C:\m\a.gguf", None), 8080, &caps);
        assert!(!args.iter().any(|a| a == "--flash-attn"));
        assert!(!args.iter().any(|a| a == "--cache-type-k"));
        assert!(!args.iter().any(|a| a == "--cache-type-v"));
        assert_eq!(value_after(&args, "--ctx-size"), Some("4096"));
    }

    #[test]
    fn build_args_drops_explicit_on_flash_when_binary_unsupported() {
        // User forces flash On but the binary has no flag: it must not appear, quantized KV
        // is dropped to f16, and context is still pinned. (The arm also logs a warn so the
        // ignored setting is diagnosable; the args are what we assert here.)
        let caps = SidecarCaps {
            flash_attn_kind: FlashAttnKind::Unsupported,
            ..caps_full()
        };
        let mut s = spec(r"C:\m\a.gguf", None);
        s.flash_attn = FlashAttnSetting::On;
        let args = build_args(&s, 8080, &caps);
        assert!(!args.iter().any(|a| a == "--flash-attn"));
        assert!(!args.iter().any(|a| a == "--cache-type-k"));
        assert!(!args.iter().any(|a| a == "--cache-type-v"));
        assert_eq!(value_after(&args, "--ctx-size"), Some("4096"));
    }

    #[test]
    fn build_args_omits_ctx_when_unsupported() {
        let caps = SidecarCaps {
            ctx_size: false,
            ..caps_full()
        };
        let args = build_args(&spec(r"C:\m\a.gguf", None), 8080, &caps);
        assert!(!args.iter().any(|a| a == "--ctx-size"));
    }

    #[test]
    fn build_args_leaves_f16_kv_implicit() {
        // f16 is the llama.cpp default, so no --cache-type-* is passed even with full caps.
        let mut s = spec(r"C:\m\a.gguf", None);
        s.kv_cache_type = KvCacheType::F16;
        let args = build_args(&s, 8080, &caps_full());
        assert!(!args.iter().any(|a| a == "--cache-type-k"));
        assert!(!args.iter().any(|a| a == "--cache-type-v"));
    }
}
