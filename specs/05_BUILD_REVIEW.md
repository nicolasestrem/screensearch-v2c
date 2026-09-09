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

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Shipped 0.3.2 history (the P7.2 product-shell mini-arc: PR1–PR6) →
> `specs/archive/05_BUILD_REVIEW.v0.3.2.md`.
> Shipped 0.3.3 history (the browser-freeze hotfix: UIA skips Chromium/Electron windows) →
> `specs/archive/05_BUILD_REVIEW.v0.3.3.md`.
> Shipped 0.4.0 history (the P8 sessions arc: PR1–PR7, Passes 1–17) →
> `specs/archive/05_BUILD_REVIEW.v0.4.0.md`.
> Live file holds only the current arc.

---

## Pass 1 — 2026-09-09 — Approved deletion-only maintenance

- **Scope:** maintainer approved six deletions on `chore/ai-slop-cleanup`, based on
  `f58c42f7ac8eec6e3220ba86eca17f2c67bfb949`. Removed `ScreenScaffold`, `IconSearch`,
  `startOfLocalDay`, `useSetMarkNote`, the unused workspace `thiserror` declaration,
  and OCR's unused direct `tracing` dependency. Cargo regenerated only OCR's
  `tracing` edge in `Cargo.lock`; no versions changed.
- **Before edits:** TypeScript language-service references and source searches found
  no callers of the four UI symbols. Manifest/source searches found no member using
  the workspace `thiserror` declaration and no OCR source using `tracing`.
- **Preserved:** active `setMarkNote` command/overlay caller, date-range helpers,
  generated IPC bindings, existing tests, OCR implementation, schema and module
  boundaries. No new production code, dependency, abstraction, or test stub.
- **Environment correction:** inherited `NODE_ENV=production` made the first
  `npm ci` omit ESLint/TypeScript. `npm ci --include=dev` installed the locked dev
  toolchain without changing npm configuration or manifests. Windows 11, Rust
  stable MSVC 1.98.1, Node 26.8.1; CI's Node 22 was not exercised locally.
- **Observed verification:** UI `npm run lint && npm test && npm run build`; staged
  MCP sidecar; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets
  -- -D warnings`; `cargo build --workspace`; `cargo test --workspace`; binding
  freshness and `git diff --check` all passed. OCR's normal and explicitly ignored
  WinRT test also passed after removing its dependency.

Programmatic comparison of the before/after workspace logs (verbatim output;
compares test identities and outcomes, not just aggregate counts):

```text
Before: {"passed": 759, "failed": 0, "ignored": 15, "measured": 0, "filtered_out": 0}
After:  {"passed": 759, "failed": 0, "ignored": 15, "measured": 0, "filtered_out": 0}
PASS: identical Rust test identities and outcomes before/after; 774 named test executions
```

Post-edit build completion output, in MCP/clippy/build/test order:

```text
    Finished `release` profile [optimized] target(s) in 1.78s
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.48s
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.90s
    Finished `test` profile [unoptimized + debuginfo] target(s) in 34.81s
```

`cargo test -p ocr -- --ignored` (verbatim test-result excerpt):

```text
running 1 test
test tests::winrt_ocr_recognizes_blank_image ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s
```

**UI parity — not byte-identical builds.** The initial strict comparison failed:
Tailwind stopped emitting `.max-w-5xl{max-width:64rem}`, used only by the deleted
scaffold. CSS fingerprint changes propagated through JS/HTML asset references.
A bijective comparison of actual build filenames, normalizing only those asset
references, proved the sole content change is that unused CSS rule. The comparator
also rejected deliberate additional JS/CSS differences (in-memory negative controls;
neither build was modified). Verbatim output:

```text
PASS: 32 baseline files and 32 rebuilt files map one-to-one
PASS: 31 files identical after asset-filename reference normalization
PASS: only CSS rule removed: .max-w-5xl{max-width:64rem}
PASS: negative control rejects additional .js content changes
PASS: negative control rejects additional .css content changes
```

**Evidence:** full local logs in `%LOCALAPPDATA%\Temp\screensearch-cleanup-` with
suffixes `ui-baseline.log`, `rust-baseline.log`, `ocr-live-baseline.log`,
`ui-after.log`, `rust-after.log`, `ocr-after.log`, and `ui-parity.log`.
The `ui-after.log` includes the initial strict-parity assertion failure; it is not
hidden by the later explained comparison. The saved baseline build and comparator
live in `%LOCALAPPDATA%\Temp\screensearch-cleanup-baseline-f58c42f\`.
A final full rerun also exited 0; complete output is in
`%LOCALAPPDATA%\Temp\screensearch-cleanup-final-verification.log`. Its workspace
test identities and outcomes still match the unchanged baseline (comparison output
in `screensearch-cleanup-final-comparison.log` in the same directory).

**Still risky / deferred:** only two UI tests exist; this is not a full interactive
UI, GPU/model, capture, or installer audit. The workspace still ignores 15 tests;
OCR's one ignored test was additionally exercised explicitly. Existing npm audit
findings are tracked in `07`; no dependency upgrade or broader refactor was attempted.
**Independent static review:** Hermes reviewer `deleg_0aedecd1` (`gpt-6-astra`)
returned a complete JSON verdict with `passed: true`, no security concerns and no
logic errors. The parent parsed the durable verdict and verified that the current
seven-file production/dependency diff still exactly matches the reviewed snapshot
(zero additions, 53 deletions). The verdict is stored as `review-verdict.json` in
the evidence directory above. This static review did not run or stand in for the
parent's separate build/test checks or interactive Windows smoke testing. At the
end of this verification pass, no PR, remote CI run, release or merge was performed.

## Pass 2 — 2026-09-09 — Local release installer

- **Requested:** build the reviewed cleanup for hands-on testing, without installing,
  launching, publishing or changing application behavior.
- **Build:** installed the root's locked Tauri CLI with `npm ci --include=dev`.
  The existing root build script staged the real MCP sidecar, rebuilt/typechecked
  the UI, compiled the optimized Windows application, and produced the NSIS bundle.
- **Local-only signing override:** a temporary file outside the repository,
  `%LOCALAPPDATA%\Temp\screensearch-local-build.json`, contained:

```json
{"bundle":{"createUpdaterArtifacts":false}}
```

The command below ran from the repository root in Git Bash (exit 0):

```bash
npm run build -- --ci --no-sign --config "$LOCALAPPDATA/Temp/screensearch-local-build.json"
```

The override disables updater-artifact generation for this invocation only;
`--no-sign` explicitly skips signing. The checked-in Tauri configuration, updater
public key/endpoint, app identity and version remain unchanged. This is a local
unsigned installer, not a signed auto-update release or an isolated test profile.

Verbatim build-output excerpts:

```text
    Finished `release` profile [optimized] target(s) in 4m 16s
       Built application at: C:\Users\nicol\screensearch-v2c\target\release\screensearch.exe
```

```text
    Finished 1 bundle at:
        C:\Users\nicol\screensearch-v2c\target\release\bundle\nsis\ScreenSearch_0.4.0_x64-setup.exe
```

- **Artifact:** `target/release/bundle/nsis/ScreenSearch_0.4.0_x64-setup.exe`;
  13,695,933 bytes (13.06 MiB).
- **SHA-256:** `cce7bebcb7efcfedde0156dd1728870f5d4fb1cf9a895f64400707526e85b281`.
- **Independent artifact checks:** parsed the installer's PE header and verified
  the application is an x64 PE. Compared the full tracked diff with the pre-package
  snapshot; packaging did not mutate tracked files. Verbatim output:

```text
PASS: installer PE and x64 application artifact verified
PASS: tracked repository diff unchanged by packaging
```

- **Evidence:** `%LOCALAPPDATA%\Temp\screensearch-cleanup-release-build.log` retains
  complete stdout/stderr. Installer and build outputs remain git-ignored; no binary
  or signing material is committed.
- **Limits:** installer execution and interactive application QA have not been
  performed. Windows may show a SmartScreen warning. Existing application identity
  means this installer is not a side-by-side sandbox. Build success is not a claim
  of installation, runtime smoke-test, remote CI, release or merge success.
- **Delivery authorization:** after receiving the installer, the maintainer
  requested documentation, commit, branch push and an open PR. This authorizes
  that delivery only; no merge or release is authorized.

