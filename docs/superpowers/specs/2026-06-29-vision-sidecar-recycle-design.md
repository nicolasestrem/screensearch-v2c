# Design — Vision/Answer sidecar RSS-ceiling recycle valve

**Date:** 2026-06-29 · **Branch:** `fix/vision-sidecar-rss-recycle` · **Status:** approved design, pre-plan

## Context — why this exists
The bundled `llama-server` (build **9842**, 6f4f53f2b) running the Quality vision model
(`Qwen3VL-8B-Instruct-Q4_K_M` 4.68 GB + `mmproj-…-F16` 1.08 GB) has a **sustained per-inference
host-memory leak of ~149 MB per vision frame**, confirmed by an isolated controlled test on
2026-06-29 (own `llama-server`, 60 frames, no app confound):

```
baseline req= 0  Priv= 6.753GB  VRAM=8107MiB
run      req=20  Priv=10.483GB  VRAM=8283MiB
run      req=40  Priv=14.014GB  VRAM=8283MiB
run      req=60  Priv=16.303GB  VRAM=8283MiB   (idle-hold 25s: held, no drop)
post-warmup slope: 149 MB/inference, dead linear, VRAM flat
```

Committed host RAM climbs ~149 MB/frame with **VRAM flat** → a host-side leak in llama.cpp's
multimodal (mtmd/clip) path, **upstream — not in ScreenSearch code**. It is **not reclaimed while
the process is alive**; only process exit frees it. A long continuous backlog drain (idle backfill
keeps the sidecar warm, so idle-TTL never fires) is what runs a user to ~16 GB, where concurrent
requests can also crash the process. The user's reported 4.6 → 9.5 → ~16 GB matches this exactly.

**This design does not fix the upstream leak.** It adds a safety valve so the sidecar's host RAM
cannot run away during long tagging: recycle (kill + lazy respawn) the process when its committed
RAM crosses a ceiling. Eviction is already proven to fully reclaim, so a recycle resets the leak.

## Goal / non-goals
- **Goal:** bound the sidecar's host RAM during sustained tagging, automatically, on every machine,
  without interrupting user-facing requests, and without regressing the existing idle-TTL/keep-warm
  behavior.
- **Non-goals:** fixing the upstream llama.cpp leak; bumping the bundled binary (tracked separately);
  localizing the leak to a specific buffer (the isolated test already pins it to the image path).

## Decisions (locked with user)
1. **Trigger:** RSS ceiling on the sidecar's **committed/private bytes** (the metric that climbed).
2. **Default:** ON by default, **auto** threshold derived from total system RAM, with an explicit
   MB override.
3. **Scope:** **both lanes** — the valve lives in the shared `ModelSupervisor`; vision needs it now,
   answer lane gets free defense-in-depth (ceiling won't trip for a well-behaved 2.5 GB text model).
4. **A ceiling breach overrides `pinned` and `backfill_active`** — it is a memory-safety event that
   must fire during a backlog drain (the runaway case). It still drains in-flight requests first.
5. **Auto-ceiling formula (starting point, tunable):**
   `clamp(total_physical_RAM / 2, 8 GiB, total_physical_RAM − 6 GiB)`
   (31 GB → ~15.5 GB; 16 GB → ~8 GB).

## Mechanism — reuse the existing exclusive-switch path
The recycle check lives in `ModelSupervisor::acquire()` (the request entry point — exactly where
memory grows). Before returning a `Lease`:
1. Probe the running sidecar's committed bytes via an **injected probe** fn (default = Windows impl).
2. If `should_recycle(rss, ceiling, in_flight)` → perform a recycle = the **existing model-switch
   sequence applied to the same spec**: `enter_for_model_switch` drains all permits (waits for
   in-flight to clear) → `kill_and_confirm` (the single teardown path → no orphan, VRAM freed) →
   respawn fresh → serve the request on the new process.
3. Emit `SidecarState::Recycled`; log `sidecar recycled at RSS ceiling rss=… ceiling=…`.

Recycle deliberately bypasses the `backfill_active`/`pinned` keep-warm gates (unlike idle eviction)
but always drains in-flight first. The triggering background job absorbs the ~30s cold reload; no
user-facing request is torn out (drain guarantees it).

## Trigger — pure predicate + injectable probe (testability)
- `pub fn should_recycle(rss_bytes: u64, ceiling_bytes: u64, in_flight: usize) -> bool`
  `= in_flight == 0 && ceiling_bytes > 0 && rss_bytes >= ceiling_bytes` — extracted like the
  existing `should_evict` / `idle_expired`; unit-tested at boundaries (over / under / equal /
  disabled / in-flight>0).
- `process::query_private_bytes(pid) -> Option<u64>` using `GetProcessMemoryInfo` →
  `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage` (matches PowerShell `PrivateMemorySize64`, the metric
  that climbed). Reuses the `PROCESS_QUERY_LIMITED_INFORMATION` open already in `process.rs`.
- `process::total_physical_ram() -> u64` via `GlobalMemoryStatusEx` for the auto ceiling.
- The supervisor stores the probe as a field (fn pointer / boxed fn), defaulted to the real impl,
  overridden in tests so unit tests drive RSS values deterministically.

## Config + UI
- New settings keys in the existing `sidecar.*` group:
  - `sidecar.recycle_enabled` — bool, default **true**.
  - `sidecar.recycle_rss_mb` — u32, **0 = auto** (derive from RAM), explicit = fixed MB ceiling,
    clamped (e.g. floor 4096 MB when explicit; 0 stays auto).
- Plumbing: `crates/traits/src/ipc.rs` (`Settings` fields + doc comments + `SupervisorConfig`/
  `SidecarParams` wiring) → `crates/kernel/src/settings.rs` (clamp/defaults) → `src-tauri/src/lib.rs`
  (resolve auto ceiling, build `SupervisorConfig`, hot-apply on settings change like other knobs).
- UI: Settings screen gains a toggle + MB field under the sidecar group; typed IPC via ts-rs —
  **regenerate `ui/src/bindings/` and commit** (binding guard). All view states defined per UI rules.
- **No DB schema bump** (settings are key-value; missing key → default).

## Observability
- Add `SidecarState::Recycled` to the status enum; `emit` it on recycle. StatusRail surfaces it
  transiently (mirrors how `Evicted` is shown).

## Testing (TDD)
- Pure: `should_recycle` boundary table; auto-ceiling derivation (small/large RAM, clamps).
- Supervisor integration (injected RSS probe): drive RSS over ceiling → assert recycle drains
  in-flight, routes through `kill_and_confirm` (no orphan), respawns, emits `Recycled`; under
  ceiling → no recycle; `recycle_enabled=false` → never recycles; ceiling breach fires even when
  `pinned`/`backfill_active` are set.
- Existing idle-TTL / keep-warm tests must stay green (recycle is additive, not a replacement).

## Verification (end-to-end, paste raw output)
1. UI: `cd ui && npm ci && npm run lint && npm run build`.
2. Rust: `cargo fmt --all -- --check` · `cargo clippy --workspace --all-targets -- -D warnings` ·
   `cargo build --workspace` · `cargo test --workspace`.
3. Binding guard: `git diff --exit-code -- ui/src/bindings` clean.
4. Live proof: set `sidecar.recycle_rss_mb` to a low value (e.g. 9000), run a vision backlog, and
   confirm via Task Manager / `Get-Process llama-server` that committed RAM rises to the ceiling,
   the sidecar recycles (PID changes, RAM resets), and tagging continues — paste the before/after.

## Files to touch
- `crates/inference/src/supervisor.rs` — recycle in `acquire`, `should_recycle` predicate, probe
  field, `Recycled` emit.
- `crates/inference/src/process.rs` — `query_private_bytes`, `total_physical_ram`.
- `crates/traits/src/ipc.rs` — `Settings` fields, `SupervisorConfig` ceiling, `SidecarStatus`/state,
  doc comments, ts-rs.
- `crates/kernel/src/settings.rs` — clamps/defaults for the new keys.
- `src-tauri/src/lib.rs` — resolve auto ceiling, wire config, hot-apply.
- `ui/` — Settings controls + regenerated bindings.
- Docs: `specs/03_MASTER_PRODUCTION_SPEC.md §8`, `specs/05/06/08`, `CHANGELOG.md`,
  `specs/07_KNOWN_GAPS.md` (upstream leak + mitigation).
