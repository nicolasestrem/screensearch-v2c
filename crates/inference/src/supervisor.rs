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
use traits::{SidecarState, SidecarStatus};

use crate::client::SidecarClient;
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
                    // model switch).
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
                Some(_) => {
                    drop(guard);
                    drop(permit);

                    let permit = self.gate.enter_for_model_switch().await?;
                    let mut guard = self.state.lock().await;
                    if let Some(running) = guard.as_ref() {
                        if !needs_restart(&running.spec, &spec) {
                            if running_sidecar_healthy(running).await {
                                drop(guard);
                                drop(permit);
                                continue;
                            }
                            self.emit(SidecarState::Crashed, Some(model_label(&running.spec)));
                        }
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
    /// memory is freed before the next model spawns. Polls rather than blocking the
    /// executor on `WaitForSingleObject`.
    async fn stop_child(&self, old: SidecarProcess) {
        let pid = old.child.pid();
        old.child.kill();
        let deadline = Instant::now() + PROCESS_EXIT_WAIT;
        while process::pid_alive(pid) && Instant::now() < deadline {
            tokio::time::sleep(HEALTH_POLL).await;
        }
        let _ = std::fs::remove_file(&self.config.pidfile);
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
            p.child.kill();
            let _ = std::fs::remove_file(&self.config.pidfile);
        }
        self.emit(SidecarState::Stopped, None);
    }

    /// Spawns `llama-server` for `spec`, binds it to the job **before** resuming, then
    /// waits for `/health`. On any failure the child is killed so nothing leaks.
    async fn spawn_for(&self, spec: &ModelSpec) -> Result<SidecarProcess> {
        let port = free_port().context("allocate sidecar port")?;
        let args = build_args(spec, port);
        self.emit(SidecarState::Starting, Some(model_label(spec)));

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
                self.emit(SidecarState::Crashed, None);
                bail!("llama-server exited during startup");
            }
            if client.health().await {
                break;
            }
            if Instant::now() >= deadline {
                child.kill();
                self.emit(SidecarState::Crashed, None);
                bail!(
                    "llama-server did not become healthy within {:?}",
                    self.config.health_timeout
                );
            }
            tokio::time::sleep(HEALTH_POLL).await;
        }

        self.emit(SidecarState::Ready, Some(model_label(spec)));
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

    /// Stops the sidecar if it has been idle past the TTL and nothing is in flight.
    async fn maybe_evict(&self) {
        if self.in_flight.load(Ordering::SeqCst) > 0 {
            return;
        }
        let elapsed = self
            .last_activity
            .lock()
            .map(|g| g.elapsed())
            .unwrap_or_default();
        if !idle_expired(elapsed, self.config.idle_ttl) {
            return;
        }
        let mut guard = self.state.lock().await;
        // Re-check under the lock — a request may have arrived between checks.
        if self.in_flight.load(Ordering::SeqCst) > 0 {
            return;
        }
        if let Some(p) = guard.take() {
            p.child.kill();
            let _ = std::fs::remove_file(&self.config.pidfile);
            self.emit(SidecarState::Evicted, None);
            tracing::info!("sidecar evicted after idle TTL");
        }
    }

    fn emit(&self, state: SidecarState, model: Option<String>) {
        let _ = self.events.send(SidecarStatus { state, model });
    }
}

/// Whether switching from `running` to `requested` requires a sidecar restart (a
/// different GGUF or projector). Same model → reuse the running process.
pub fn needs_restart(running: &ModelSpec, requested: &ModelSpec) -> bool {
    running.gguf_path != requested.gguf_path || running.mmproj_path != requested.mmproj_path
}

/// Pure idle predicate (extracted for testing): idle once `elapsed >= ttl`.
pub fn idle_expired(elapsed: Duration, ttl: Duration) -> bool {
    elapsed >= ttl
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
/// GPU offload, plus the same-repo projector on the vision lane.
fn build_args(spec: &ModelSpec, port: u16) -> Vec<String> {
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
    args
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
        }
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
    fn build_args_adds_mmproj_only_for_vision() {
        let answer = build_args(&spec(r"C:\m\a.gguf", None), 8080);
        assert!(!answer.iter().any(|a| a == "--mmproj"));
        assert!(answer
            .windows(2)
            .any(|w| w[0] == "--port" && w[1] == "8080"));

        let vision = build_args(&spec(r"C:\m\v.gguf", Some(r"C:\m\mmproj.gguf")), 8080);
        assert!(vision
            .windows(2)
            .any(|w| w[0] == "--mmproj" && w[1] == r"C:\m\mmproj.gguf"));
    }
}
