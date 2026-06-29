# Vision/Answer Sidecar RSS-Ceiling Recycle Valve — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cap the llama-server sidecar's committed host RAM during long tagging by recycling (kill + respawn) the process when it crosses an auto-derived ceiling — mitigating a confirmed ~149 MB/frame upstream multimodal leak.

**Architecture:** A recycle check is added to the top of `ModelSupervisor::acquire()`. When the running sidecar (same model) exceeds a configured committed-RAM ceiling, `acquire` reuses the existing exclusive-switch drain (`enter_for_model_switch` → `kill_and_confirm` → respawn same spec → serve), emitting a new `SidecarState::Recycled`. The ceiling is resolved once at construction (like `idle_ttl`) from two new settings. RAM is read with a new `process::query_private_bytes`; the auto ceiling from `process::total_physical_ram`.

**Tech Stack:** Rust 1.82 (workspace: `crates/inference`, `crates/traits`, `crates/kernel`, `src-tauri`), Tauri 2, `windows` crate, React/TS + Vite + ts-rs bindings.

## Global Constraints

- **Windows-only by design** — use Windows-native APIs; no cross-platform stubs.
- **Toolchain:** Rust 1.82, Node 22.
- **Build order (matches CI):** build UI first (`cd ui && npm ci && npm run lint && npm run build`) before any `cargo` command — `src-tauri`'s `generate_context!` embeds `ui/dist`.
- **Binding guard:** `cargo test` regenerates ts-rs bindings; `git diff --exit-code -- ui/src/bindings` MUST be clean — commit regenerated bindings.
- **UI:** typed IPC via ts-rs only (never hand-edit `ui/src/bindings/`); tokens only (no hardcoded hex/font/spacing); every view defines all states; Rules-of-Hooks is an error-level gate.
- **Verbatim verification:** paste raw command output (build/clippy/test/run); "done" = observed, not "compiles".
- **Branch:** `fix/vision-sidecar-rss-recycle` (already created; never commit to `main`).
- **No DB schema change** (settings are key-value; missing key → default).
- **Verification commands per task:** `cargo fmt --all -- --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo build --workspace` · `cargo test --workspace`.

### Deviations from the design doc (intentional, for consistency with existing code)
- **No injectable RSS probe / no runtime hot-apply.** The ceiling lives in `SupervisorConfig` and is resolved once at construction — exactly like the existing `idle_ttl` (which the hot-apply path also does **not** update). Changing the recycle settings therefore takes effect on the next app start. The recycle *decision* is covered by pure unit tests (`should_recycle`, `auto_recycle_ceiling`) mirroring the existing `should_evict`/`idle_expired` pattern; the `acquire` wiring is verified by the live test in Task 7 (the existing `acquire`/evictor paths are likewise not unit-tested because they need a real process).

---

### Task 1: Windows process-memory probes

**Files:**
- Modify: `crates/inference/Cargo.toml` (windows `features` list, currently lines 38–43)
- Modify: `crates/inference/src/process.rs` (add two functions + imports)

**Interfaces:**
- Produces: `pub fn query_private_bytes(pid: u32) -> Option<u64>` (committed/private bytes, matches Task Manager "Private"), `pub fn total_physical_ram() -> u64` (0 on failure).

- [ ] **Step 1: Add the two Win32 features.** In `crates/inference/Cargo.toml`, extend the windows features list:

```toml
[target.'cfg(windows)'.dependencies]
windows = { workspace = true, features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_JobObjects",
    "Win32_System_ProcessStatus",
    "Win32_System_SystemInformation",
    "Win32_System_Threading",
] }
```

- [ ] **Step 2: Write failing tests** at the bottom of `crates/inference/src/process.rs` (in its `#[cfg(test)] mod tests`, or add one):

```rust
#[test]
fn total_physical_ram_is_nonzero() {
    assert!(super::total_physical_ram() > 0, "GlobalMemoryStatusEx should report RAM");
}

#[test]
fn query_private_bytes_reports_for_self() {
    let me = std::process::id();
    let bytes = super::query_private_bytes(me).expect("own process has committed bytes");
    assert!(bytes > 0);
}

#[test]
fn query_private_bytes_is_none_for_pid_zero() {
    assert_eq!(super::query_private_bytes(0), None);
}
```

- [ ] **Step 3: Run tests, verify they FAIL** (functions not defined):

Run: `cargo test -p inference process::tests::query_private_bytes_reports_for_self`
Expected: FAIL — `cannot find function 'query_private_bytes'`.

- [ ] **Step 4: Implement both functions** in `crates/inference/src/process.rs`. Add to the top import block:

```rust
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
```

Add the functions (mirror the existing `OpenProcess` → SAFETY → `CloseHandle` idiom from `pid_alive`):

```rust
/// The process's committed/private bytes (`PROCESS_MEMORY_COUNTERS_EX.PrivateUsage` —
/// the metric Task Manager shows as "Private" and the recycle valve bounds). `None` if
/// the process is gone or unqueryable.
pub fn query_private_bytes(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    // SAFETY: returns an owned handle or an error; nothing is dereferenced unsafely.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: `handle` is valid; `counters`/`cb` are a valid out-param + its size. The
    // EX struct is layout-compatible with the base counters the API writes into.
    let res = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            cb,
        )
    };
    // SAFETY: close the handle we opened.
    unsafe {
        let _ = CloseHandle(handle);
    }
    res.ok()?;
    Some(counters.PrivateUsage as u64)
}

/// Total physical RAM in bytes (`GlobalMemoryStatusEx.ullTotalPhys`); `0` on failure.
pub fn total_physical_ram() -> u64 {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` is a valid out-param with `dwLength` set per the API contract.
    if unsafe { GlobalMemoryStatusEx(&mut status) }.is_ok() {
        status.ullTotalPhys
    } else {
        0
    }
}
```

- [ ] **Step 5: Run tests, verify they PASS:**

Run: `cargo test -p inference process::tests::`
Expected: PASS (3 new tests green).

- [ ] **Step 6: fmt + clippy + commit:**

```bash
cargo fmt --all
cargo clippy -p inference --all-targets -- -D warnings
git add crates/inference/Cargo.toml crates/inference/src/process.rs
git commit -m "feat(inference): Win32 query_private_bytes + total_physical_ram probes"
```

---

### Task 2: Pure recycle math

**Files:**
- Modify: `crates/inference/src/supervisor.rs` (add three pure fns near `should_evict` ~line 566, + tests in its test module)
- Modify: `crates/inference/src/lib.rs` (re-export the new pure fns alongside the existing `SupervisorConfig`/`should_evict` `pub use`)

**Interfaces:**
- Consumes: `process::total_physical_ram` (Task 1).
- Produces: `pub fn should_recycle(rss_bytes: u64, ceiling_bytes: u64) -> bool`; `pub fn auto_recycle_ceiling(total_ram_bytes: u64) -> u64`; `pub fn resolve_recycle_ceiling(enabled: bool, rss_mb: u32) -> u64`.

- [ ] **Step 1: Write failing tests** in the `#[cfg(test)] mod tests` of `crates/inference/src/supervisor.rs`:

```rust
const GIB: u64 = 1 << 30;

#[test]
fn should_recycle_only_at_or_above_ceiling() {
    assert!(!super::should_recycle(9 * GIB, 10 * GIB));
    assert!(super::should_recycle(10 * GIB, 10 * GIB));
    assert!(super::should_recycle(11 * GIB, 10 * GIB));
}

#[test]
fn should_recycle_disabled_when_ceiling_zero() {
    assert!(!super::should_recycle(64 * GIB, 0));
}

#[test]
fn auto_ceiling_is_half_ram_within_band() {
    assert_eq!(super::auto_recycle_ceiling(32 * GIB), 16 * GIB); // big box: RAM/2
    assert_eq!(super::auto_recycle_ceiling(16 * GIB), 8 * GIB);  // RAM/2 == floor
    assert_eq!(super::auto_recycle_ceiling(12 * GIB), 6 * GIB);  // constrained: RAM-6
    assert_eq!(super::auto_recycle_ceiling(8 * GIB), 3 * GIB);   // tiny: floored, no panic
}

#[test]
fn resolve_ceiling_branches() {
    assert_eq!(super::resolve_recycle_ceiling(false, 0), 0);        // disabled
    assert_eq!(super::resolve_recycle_ceiling(false, 9000), 0);     // disabled wins
    assert_eq!(super::resolve_recycle_ceiling(true, 9000), 9000 * 1024 * 1024); // explicit MiB
    assert!(super::resolve_recycle_ceiling(true, 0) > 0);           // auto from real RAM
}
```

- [ ] **Step 2: Run tests, verify they FAIL:**

Run: `cargo test -p inference supervisor::tests::should_recycle_only_at_or_above_ceiling`
Expected: FAIL — `cannot find function 'should_recycle'`.

- [ ] **Step 3: Implement the three fns** in `crates/inference/src/supervisor.rs`, next to `should_evict`/`idle_expired`:

```rust
/// Pure recycle predicate (extracted for testing, like `should_evict`): recycle once the
/// sidecar's committed RAM reaches the ceiling. `ceiling_bytes == 0` disables recycling.
pub fn should_recycle(rss_bytes: u64, ceiling_bytes: u64) -> bool {
    ceiling_bytes != 0 && rss_bytes >= ceiling_bytes
}

/// Auto recycle ceiling from total physical RAM: half of RAM, clamped to
/// `[8 GiB, RAM − 6 GiB]` so it sits above the vision model's ~6.8 GB load baseline yet
/// leaves headroom. Below ~14 GiB the band inverts, so fall back to the headroom bound
/// (floored at 3 GiB) — the 8B vision model wants the Default 4B tier on such machines.
pub fn auto_recycle_ceiling(total_ram_bytes: u64) -> u64 {
    const GIB: u64 = 1 << 30;
    let half = total_ram_bytes / 2;
    let lo = 8 * GIB;
    let hi = total_ram_bytes.saturating_sub(6 * GIB);
    if hi <= lo {
        hi.max(3 * GIB)
    } else {
        half.clamp(lo, hi)
    }
}

/// Resolve the configured recycle settings to a byte ceiling for `SupervisorConfig`:
/// `0` when disabled; an explicit MiB value; otherwise auto-derived from total RAM.
pub fn resolve_recycle_ceiling(enabled: bool, rss_mb: u32) -> u64 {
    if !enabled {
        return 0;
    }
    if rss_mb > 0 {
        return (rss_mb as u64) * 1024 * 1024;
    }
    auto_recycle_ceiling(process::total_physical_ram())
}
```

- [ ] **Step 4: Re-export from the crate root.** In `crates/inference/src/lib.rs`, find the existing `pub use ... supervisor::{... SupervisorConfig ...}` (or `pub use supervisor::*`) and ensure `should_recycle`, `auto_recycle_ceiling`, `resolve_recycle_ceiling` are exported the same way `should_evict`/`SupervisorConfig` are. (Grep: `pub use` in that file; mirror the existing line.)

- [ ] **Step 5: Run tests, verify they PASS:**

Run: `cargo test -p inference supervisor::tests::`
Expected: PASS (4 new tests green).

- [ ] **Step 6: fmt + clippy + commit:**

```bash
cargo fmt --all
cargo clippy -p inference --all-targets -- -D warnings
git add crates/inference/src/supervisor.rs crates/inference/src/lib.rs
git commit -m "feat(inference): pure recycle-ceiling math (should_recycle/auto/resolve)"
```

---

### Task 3: SupervisorConfig ceiling + acquire() recycle branch + Recycled state

**Files:**
- Modify: `crates/traits/src/ipc.rs` (`SidecarState` enum ~lines 837–847 — add `Recycled`)
- Modify: `crates/inference/src/supervisor.rs` (`SupervisorConfig` field; two helper methods; recycle branch at top of `acquire` loop)
- Modify: any `SupervisorConfig { … }` literals in tests (add the new field)
- Bindings: regenerate `ui/src/bindings/SidecarState.ts`

**Interfaces:**
- Consumes: `should_recycle` (Task 2), `process::query_private_bytes` (Task 1), existing `enter_for_model_switch`/`stop_child`/`spawn_with_retries`/`lease_from_state`/`emit`/`needs_restart`.
- Produces: `SupervisorConfig.recycle_ceiling_bytes: u64`; `SidecarState::Recycled`.

- [ ] **Step 1: Add the `Recycled` state.** In `crates/traits/src/ipc.rs`, extend the enum (append to keep ordering stable):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum SidecarState {
    Stopped,
    Starting,
    Ready,
    Evicted,
    Crashed,
    Recycled,
}
```

- [ ] **Step 2: Add the config field.** In `SupervisorConfig` (`crates/inference/src/supervisor.rs` ~line 40):

```rust
    /// Committed-RAM ceiling (bytes) that triggers a sidecar recycle; `0` disables it.
    /// Resolved once at construction from `sidecar.recycle_*` (like `idle_ttl`).
    pub recycle_ceiling_bytes: u64,
```

- [ ] **Step 3: Add the helper methods** to `impl ModelSupervisor` (near `needs_exclusive_switch`):

```rust
/// True when recycling is enabled and the running sidecar (same model as `spec`) has
/// grown past the committed-RAM ceiling. Cheap no-op when the ceiling is 0 (disabled).
async fn over_recycle_ceiling(&self, spec: &ModelSpec) -> bool {
    if self.config.recycle_ceiling_bytes == 0 {
        return false;
    }
    let guard = self.state.lock().await;
    guard
        .as_ref()
        .is_some_and(|p| !needs_restart(&p.spec, spec) && self.process_over_ceiling(p))
}

/// Probe the sidecar's committed bytes and compare against the ceiling.
fn process_over_ceiling(&self, p: &SidecarProcess) -> bool {
    process::query_private_bytes(p.child.pid())
        .is_some_and(|rss| should_recycle(rss, self.config.recycle_ceiling_bytes))
}
```

- [ ] **Step 4: Add the recycle branch** as the FIRST statement inside the `loop {` of `acquire` (before `if self.needs_exclusive_switch(&spec).await {`):

```rust
        // Memory safety valve: if the running sidecar (same model) has grown past the
        // recycle ceiling, refresh it before serving — drain in-flight, kill (freeing the
        // leaked host RAM + VRAM via the one teardown path), respawn the same spec. This
        // is the only teardown that fires *during* a backfill drain (the leak's runaway
        // case), so it ignores keep-warm/pin; the immediate respawn keeps the model
        // resident. The `!needs_restart` guard means a genuine model switch falls through
        // to the exclusive-switch branch below instead.
        if self.over_recycle_ceiling(&spec).await {
            let permit = self.gate.enter_for_model_switch().await?;
            let mut guard = self.state.lock().await;
            // Re-check under the exclusive lock: another caller may have already recycled
            // or switched, or the process may have been replaced since the probe.
            if guard
                .as_ref()
                .is_some_and(|p| !needs_restart(&p.spec, &spec) && self.process_over_ceiling(p))
            {
                let old = guard.take().expect("sidecar present after over-ceiling check");
                self.emit(SidecarState::Recycled, Some(&old.spec));
                tracing::info!(
                    ceiling_bytes = self.config.recycle_ceiling_bytes,
                    "sidecar recycled at committed-RAM ceiling"
                );
                self.stop_child(old).await;
                let proc = self.spawn_with_retries(&spec).await?;
                *guard = Some(proc);
                let lease = self
                    .lease_from_state(guard.as_ref().expect("sidecar present after recycle"), permit.into_single());
                drop(guard);
                return Ok(lease);
            }
            drop(guard);
            drop(permit);
            continue;
        }
```

- [ ] **Step 5: Fix all `SupervisorConfig` literals.** Grep and add `recycle_ceiling_bytes: 0,` to each (tests + the real construction is updated in Task 4):

Run: `git grep -n "SupervisorConfig {"`
For every match in a test/helper, add `recycle_ceiling_bytes: 0,`.

- [ ] **Step 6: Build + regenerate bindings + verify green:**

```bash
cargo build --workspace
cargo test --workspace
```
Expected: build PASS, tests PASS (existing idle-TTL/keep-warm tests still green; recycle wiring compiles).

- [ ] **Step 7: Confirm the binding regenerated and commit:**

```bash
git diff --name-only -- ui/src/bindings   # expect ui/src/bindings/SidecarState.ts changed
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/traits/src/ipc.rs crates/inference/src/supervisor.rs ui/src/bindings/SidecarState.ts
git commit -m "feat(inference): recycle sidecar at committed-RAM ceiling in acquire()"
```

---

### Task 4: Settings plumbing (Rust) + supervisor wiring

**Files:**
- Modify: `crates/traits/src/ipc.rs` (`Settings` struct fields + doc comments ~lines 424–462; `Default` impl ~lines 626–637)
- Modify: `crates/kernel/src/settings.rs` (load ~line 70, clamp in `sanitize_settings` ~line 524, save ~line 333)
- Modify: `src-tauri/src/lib.rs` (`SupervisorConfig` construction ~lines 1128–1135)
- Bindings: regenerate `ui/src/bindings/Settings.ts`

**Interfaces:**
- Consumes: `inference::resolve_recycle_ceiling` (Task 2), `SupervisorConfig.recycle_ceiling_bytes` (Task 3), existing `boolean`/`num`/`bool_str`/`clamp_u32` helpers.
- Produces: `Settings.sidecar_recycle_enabled: bool`, `Settings.sidecar_recycle_rss_mb: u32`.

- [ ] **Step 1: Write a failing clamp test** in `crates/kernel/src/settings.rs` test module:

```rust
#[test]
fn recycle_rss_mb_clamps_explicit_but_keeps_auto_zero() {
    let mut s = Settings::default();
    s.sidecar_recycle_rss_mb = 0;
    sanitize_settings(&mut s);
    assert_eq!(s.sidecar_recycle_rss_mb, 0, "0 stays auto");

    s.sidecar_recycle_rss_mb = 100; // below 4096 floor
    sanitize_settings(&mut s);
    assert_eq!(s.sidecar_recycle_rss_mb, 4096);

    s.sidecar_recycle_rss_mb = 999_999; // above max
    sanitize_settings(&mut s);
    assert_eq!(s.sidecar_recycle_rss_mb, 131_072);
}
```
(If `sanitize_settings` is private/named differently, match the existing sidecar clamp test's call — grep `fn sanitize_settings`.)

- [ ] **Step 2: Run it, verify FAIL** (field missing):

Run: `cargo test -p kernel recycle_rss_mb_clamps_explicit_but_keeps_auto_zero`
Expected: FAIL — no field `sidecar_recycle_rss_mb`.

- [ ] **Step 3: Add the `Settings` fields.** In `crates/traits/src/ipc.rs`, after `sidecar_flash_attn`:

```rust
    /// Recycle (restart) the sidecar when its committed host RAM crosses the ceiling,
    /// reclaiming the upstream llama.cpp multimodal leak that otherwise grows the vision
    /// sidecar ~150 MB per frame during long tagging. On by default.
    pub sidecar_recycle_enabled: bool,
    /// Committed-RAM ceiling in MiB that triggers a sidecar recycle. `0` = automatic
    /// (derived from total system RAM). Ignored when `sidecar_recycle_enabled` is false.
    pub sidecar_recycle_rss_mb: u32,
```

In `Default for Settings`, after `sidecar_flash_attn: FlashAttnSetting::Auto,`:

```rust
            sidecar_recycle_enabled: true,
            sidecar_recycle_rss_mb: 0,
```

- [ ] **Step 4: Load + clamp + save** in `crates/kernel/src/settings.rs`.

Load (next to `sidecar_ctx_size`):
```rust
        sidecar_recycle_enabled: boolean(store, "sidecar.recycle_enabled", d.sidecar_recycle_enabled).await,
        sidecar_recycle_rss_mb: num(store, "sidecar.recycle_rss_mb", d.sidecar_recycle_rss_mb).await,
```

Clamp (in `sanitize_settings`, after the `sidecar_ctx_size` clamp):
```rust
    // 0 = automatic (derived from RAM); any explicit value is a real MiB ceiling clamped
    // to a sane band (4 GiB floor so it clears the model's load baseline).
    s.sidecar_recycle_rss_mb = if s.sidecar_recycle_rss_mb == 0 {
        0
    } else {
        clamp_u32(s.sidecar_recycle_rss_mb, 4096, 131_072)
    };
```

Save (next to `sidecar.ctx_size`):
```rust
        (
            "sidecar.recycle_enabled".into(),
            bool_str(s.sidecar_recycle_enabled).into(),
        ),
        (
            "sidecar.recycle_rss_mb".into(),
            s.sidecar_recycle_rss_mb.to_string(),
        ),
```

- [ ] **Step 5: Wire the ceiling** in `src-tauri/src/lib.rs` `SupervisorConfig` construction (~line 1128):

```rust
    let config = SupervisorConfig {
        binary,
        reap_binaries,
        pidfile,
        idle_ttl: Duration::from_secs(settings.sidecar_idle_ttl_secs as u64),
        health_timeout: SIDECAR_HEALTH_TIMEOUT,
        caps,
        recycle_ceiling_bytes: inference::resolve_recycle_ceiling(
            settings.sidecar_recycle_enabled,
            settings.sidecar_recycle_rss_mb,
        ),
    };
```
(If the `SupervisorConfig` here is built per-lane/twice, add the field to each. If `inference` is imported under a different alias, match it — grep `use inference` / `SupervisorConfig` in the file.)

- [ ] **Step 6: Run tests + verify bindings regenerated:**

```bash
cargo test --workspace
git diff --name-only -- ui/src/bindings   # expect ui/src/bindings/Settings.ts changed
```
Expected: the new clamp test PASSES; `Settings.ts` now has `sidecar_recycle_enabled`/`sidecar_recycle_rss_mb`.

- [ ] **Step 7: fmt + clippy + commit:**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/traits/src/ipc.rs crates/kernel/src/settings.rs src-tauri/src/lib.rs ui/src/bindings/Settings.ts
git commit -m "feat(settings): sidecar.recycle_enabled + recycle_rss_mb, wire ceiling"
```

---

### Task 5: UI — Settings controls + status label

**Files:**
- Modify: `ui/src/routes/Settings.tsx` (sidecar panel ~lines 920–999; `sanitizeSettings` clamps ~lines 103–107)
- Modify: the component that renders `SidecarState` (grep below) to handle `"recycled"`

**Interfaces:**
- Consumes: `Settings.ts` bindings (Task 4), `SidecarState.ts` (Task 3), existing `Toggle`/`Field`/`set`/`intHandler`/`clampInt`/`APPLY_SIDECAR`.

- [ ] **Step 1: Add the two controls** to the sidecar `Panel` in `ui/src/routes/Settings.tsx` (place near the ctx-size `Field`):

```tsx
          <Toggle
            label="Recycle sidecar on high memory"
            checked={draft.sidecar_recycle_enabled}
            onChange={(v) => set("sidecar_recycle_enabled", v)}
            hint={`Restarts the engine when its memory crosses the ceiling, reclaiming a known upstream leak that grows the vision engine over long tagging runs. ${APPLY_SIDECAR}`}
          />
          <Field
            label="Recycle memory ceiling (MiB)"
            type="number"
            min={0}
            value={draft.sidecar_recycle_rss_mb}
            onChange={intHandler("sidecar_recycle_rss_mb")}
            hint={`Committed RAM that triggers a recycle. 0 — or clearing the field — = automatic (derived from your total RAM). Only used when the toggle is on. ${APPLY_SIDECAR}`}
          />
```

- [ ] **Step 2: Add the sanitize clamps** in `sanitizeSettings` (next to the `sidecar_ctx_size` line):

```tsx
    sidecar_recycle_enabled: !!s.sidecar_recycle_enabled,
    sidecar_recycle_rss_mb: s.sidecar_recycle_rss_mb === 0 ? 0 : clampInt(s.sidecar_recycle_rss_mb, 4096, 131_072),
```

- [ ] **Step 3: Handle the new sidecar state.** Locate where `SidecarState` values are mapped to labels/UI (StatusRail or readiness panel):

Run: `git grep -nE "\"evicted\"|'evicted'|SidecarState" -- ui/src`
Add a `"recycled"` case wherever `"evicted"` is handled, labeled e.g. "Recycling…" and styled like the existing transient `"starting"`/`"evicted"` state (tokens only — copy the neighboring case's classes; no new hex). If the mapping is a non-exhaustive object/switch with a default, confirm the default renders sensibly and still add an explicit case for a correct label.

- [ ] **Step 4: Lint + build the UI:**

```bash
cd ui && npm run lint && npm run build
```
Expected: lint clean (Rules-of-Hooks), build succeeds.

- [ ] **Step 5: Commit:**

```bash
cd .. && git add ui/src/routes/Settings.tsx ui/src
git commit -m "feat(ui): recycle-valve settings controls + recycled status label"
```

---

### Task 6: Docs

**Files:**
- Modify: `specs/03_MASTER_PRODUCTION_SPEC.md` (§8 settings table — add the two keys)
- Modify: `specs/07_KNOWN_GAPS.md`, `specs/05_BUILD_REVIEW.md`, `specs/06_PATCH_PLAN.md`, `specs/08_CHANGELOG_AI.md`, `CHANGELOG.md`

- [ ] **Step 1: Settings reference.** In `specs/03_MASTER_PRODUCTION_SPEC.md §8`, add rows mirroring `sidecar.ctx_size`:
  - `sidecar.recycle_enabled` — bool, default true — recycle the sidecar when committed RAM crosses the ceiling.
  - `sidecar.recycle_rss_mb` — u32, default 0 (auto), explicit clamped 4096–131072 — the ceiling in MiB.

- [ ] **Step 2: Known gap.** In `specs/07_KNOWN_GAPS.md`, record the upstream leak + mitigation:
  > **llama.cpp multimodal host-memory leak (build 9842).** Confirmed ~149 MB committed host RAM leaked per vision inference (VRAM flat; isolated 60-frame test 2026-06-29: Priv 6.8→16.3 GB). Not reclaimed while resident; freed on process exit. Mitigated locally by the `sidecar.recycle_*` valve (recycle at a committed-RAM ceiling). Real fix is upstream / a newer bundled llama.cpp build — track separately.

- [ ] **Step 3: Build-loop logs + CHANGELOG.** Add concise entries to `specs/05_BUILD_REVIEW.md`, `06_PATCH_PLAN.md`, `08_CHANGELOG_AI.md`, and a `CHANGELOG.md` line under the 0.2.x arc: "Sidecar recycle valve: recycle llama-server at a committed-RAM ceiling to bound the upstream multimodal memory leak."

- [ ] **Step 4: Commit:**

```bash
git add specs/ CHANGELOG.md
git commit -m "docs: record sidecar recycle valve + upstream multimodal leak (known gap)"
```

---

### Task 7: Live end-to-end verification

**Files:** none (runtime proof; paste raw output per the verbatim rule).

- [ ] **Step 1: Full gate (matches CI).** From repo root:

```bash
cd ui && npm ci && npm run lint && npm run build && cd ..
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
git diff --exit-code -- ui/src/bindings
```
Paste all output. All must pass and the binding diff must be empty.

- [ ] **Step 2: Force a recycle.** Launch the app (`npm run tauri dev`). In Settings set **Recycle memory ceiling** to a low value (e.g. `9000` MiB) and keep the toggle on; select the **Quality (8B)** vision tier; generate/allow a vision backlog so tagging runs continuously.

- [ ] **Step 3: Observe + capture.** In PowerShell, watch the sidecar:

```powershell
1..40 | ForEach-Object { $p = Get-Process -Name llama-server -ErrorAction SilentlyContinue | Select-Object -First 1; if ($p) { "{0} PID={1} Priv={2:N2}GB" -f (Get-Date -Format HH:mm:ss),$p.Id,($p.PrivateMemorySize64/1GB) }; Start-Sleep 5 }
```
Expected and to paste: committed RAM climbs toward ~9 GB, then the **PID changes** (recycle: kill + respawn) and committed RAM drops back to the ~6.8 GB baseline, while tagging continues. The app log shows `sidecar recycled at committed-RAM ceiling`.

- [ ] **Step 4: Record the result** in `specs/08_CHANGELOG_AI.md` (and the plan file) with the pasted before/after PID + RAM, then commit if anything changed.

---

## Self-Review

**Spec coverage:** RSS-ceiling trigger → Tasks 2–3; on-by-default + auto threshold → Tasks 2 (`auto_recycle_ceiling`/`resolve_recycle_ceiling`) + 4 (defaults); both lanes → Task 4 (per-lane `SupervisorConfig`); override pin/backfill → Task 3 (branch ignores keep-warm gates); committed-bytes metric → Task 1 (`PrivateUsage`); config + UI → Tasks 4–5; observability `Recycled` → Task 3 (+ UI label Task 5); tests → Tasks 1,2,4; docs → Task 6; live proof → Task 7. The design's "injectable probe / hot-apply" is intentionally simplified (see Deviations) — covered and called out.

**Placeholder scan:** no TBD/TODO; every code step shows full code; "follow the existing pattern" appears only for re-export lines and the UI state-label case, each with an exact grep to locate and a named neighbor to mirror.

**Type consistency:** `recycle_ceiling_bytes: u64` (config) ← `resolve_recycle_ceiling(enabled: bool, rss_mb: u32) -> u64` ← settings `sidecar_recycle_enabled: bool` / `sidecar_recycle_rss_mb: u32`; `should_recycle(u64,u64)->bool` used by `process_over_ceiling`; `SidecarState::Recycled` emitted in `acquire` and labeled in UI. Consistent across tasks.
