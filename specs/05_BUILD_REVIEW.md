# 05 — Build Review

> **Populated during the build**, after each meaningful pass (`04 §7`). Record what actually
> happened — honestly. Empty until P0 begins.

For each build pass, append an entry:

## Pass <n> — <date> — <phase, e.g. P0 Scaffold>
- **Implemented:** what now works (with the verbatim verification output that proves it).
- **Skipped / deferred:** what was intentionally not done, and why.
- **Hallucinated / corrected:** anything the agent assumed that turned out wrong.
- **Broke / regressed:** what stopped working, and the fix.
- **Still risky:** areas that compile/pass but warrant scrutiny.

---

## Pass 1 — 2026-06-21 — P0 Scaffold

**Branch:** `p0-scaffold`. Toolchain observed: rustc/cargo 1.96.0, Node v26.3.0, npm 11.16.0,
`@tauri-apps/cli` 2.11.3 (via npm), `tauri` crate 2.11.3, `ts-rs` 10.1.0.

### Implemented (with verbatim verification)
- **Cargo workspace** (`Cargo.toml`, resolver 2) with members `src-tauri`, `crates/{traits,
  kernel, store, capture, ocr, embeddings, inference, doctor}`. Dependency rule enforced: module
  crates depend only on `traits` (`03 §2`).
- **`traits` crate** — all six contracts from `03 §3` (`CaptureSource`, `OcrProvider`,
  `EmbeddingProvider`, `VisionProvider`, `AnswerProvider`, `Store`) + domain/jobs/ipc types. IPC
  types derive `ts_rs::TS` and export to `ui/src/bindings/` via `cargo test`.
- **Skeleton module crates** — honestly-empty libs (doc comment naming the phase that fills them;
  re-export of the contract they will implement). No fake impls.
- **`src-tauri` shell** — Tauri 2 app, `get_readiness` + `ping` typed commands, capabilities,
  icons generated from `assets/icon-source.png`, console tracing.
- **UI** — React 18 + TS + Vite skeleton; P0 screen exercises typed IPC; ESLint flat config with
  the Rules-of-Hooks gate at error level.
- **CI** — `.github/workflows/ci.yml` (windows-latest): UI install/lint/build → fmt, clippy
  `-D warnings`, build, test, ts-rs binding-drift guard, doctor.
- **`doctor`** — environment smoke-check for WebView2 / Vulkan / llama-server.

Verbatim verification (run 2026-06-21):
```
$ cargo fmt --all -- --check          # exit 0 (no diff)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.73s   # exit 0
$ cargo test --workspace
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   # traits (27 export + 1 bigint guard); other crates 0 tests
$ cargo run -p doctor
[ OK ]  WebView2        Evergreen Runtime v149.0.4022.80
[ OK ]  Vulkan          vulkan-1.dll loadable (GPU acceleration available)
[WARN]  llama-server    not on PATH — bundled/downloaded for the sidecar in P4
$ cd ui && npm run build
✓ built in 380ms   (tsc --noEmit clean; dist/index.html + assets emitted)
$ cd ui && npm run lint               # exit 0 (no findings)
$ git diff --exit-code -- ui/src/bindings   # exit 0 (committed bindings current)
```
- **Observed running:** `npx tauri dev` launched the WebView2 window (title "ScreenSearch",
  app icon shown). The P0 screen rendered **"Kernel says: pong"** (the `ping` command) and the
  readiness list capture/db/embed_model/sidecar = `Unknown` (the `get_readiness` command),
  confirming the Tauri bridge + generated `ts-rs` bindings work end-to-end. Captured via
  `PrintWindow`; process verified by `MainWindowTitle = "ScreenSearch"`.

### Skipped / deferred (intentional, by phase)
- `Store`/`JobQueue`/schema → P1. WGC capture + WinRT OCR → P2. fastembed → P3. Sidecar
  Job-Object lifecycle + real inference → P4. Command Deck UI + packaging → P5.
- `03 §9` daily-rotating **file** logging — console only in P0 (file sink needs the app data dir;
  wired in P1).
- `03 §11` **tauri build artifact** job (Inno Setup installer + portable ZIP) — P5 packaging.
- `tauri.conf.json` `bundle.targets: "all"` is a placeholder; P5 switches to Inno + ZIP (`00 §G`).

### Hallucinated / corrected
- Assumed `ts-rs` `export_to` is relative to the crate manifest dir (per its docs); it actually
  anchored at the **source-file** dir here, so `../../ui/...` landed in `crates/ui/...`. Corrected
  to `../../../ui/src/bindings/`.
- Assumed `TS_RS_LARGE_INT=number` (env / `.cargo/config.toml`) would map `i64/u64` to TS
  `number`; ts-rs 10.1's macro ignored it. Corrected to per-field `#[ts(type = "number")]`
  (Tauri's JSON wire delivers 64-bit ints as JS `number`), guarded by a regression test.

### Broke / regressed
- None. (Transient: the `tauri dev` background task reports exit 127 because I terminated its
  process tree after observing the window — expected, not a failure of the app.)

### Still risky / to watch
- `csp: null` in `tauri.conf.json` (dev convenience) — must be hardened in P5 (`07`).
- Local npm `allow-scripts` policy blocked esbuild's postinstall; the build still worked (binary
  resolved via the platform optional-dependency). Could matter on locked-down dev machines.
- `forbid(unsafe_code)` is intentionally **absent** on `capture`/`ocr`/`inference` (P2/P4 need
  `unsafe` FFI) and present on the pure crates.
