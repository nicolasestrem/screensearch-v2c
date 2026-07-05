# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

## 2026-07-05 — 0.3.2 PR2: auto-update (#69; Rust lane)

- **Change:** Implemented issue #69 (auto-update) per `03 §11b` / `docs/0.3.2.md` §3 PR2 (D1/D2).
  - **Rust core.** New `crates/traits` type `UpdateStatus` (tagged enum `idle`/`checking`/
    `available`/`downloading`/`ready`/`error`, exported via ts-rs → `ui/src/bindings/UpdateStatus.ts`).
    New `src-tauri/src/update.rs`: an updater manager holding its own `UpdaterState` (status +
    downloaded `PendingUpdate` + a single-flight `AtomicBool`), three typed commands
    (`get_update_status`, `check_for_updates`, `restart_to_apply_update`), and the
    `update_status_changed` event. `src-tauri/src/lib.rs`: registers `tauri-plugin-updater`, manages
    `UpdaterState`, spawns a **release-build-only** launch check, registers the three commands, and
    **factors the `RunEvent::ExitRequested` shutdown into a shared `graceful_shutdown` helper** reused
    by install-on-restart (so quit + update-install can't drift). `src-tauri/src/main.rs`: a
    `--version`/`-V` early-return before the Builder (single-instance-safe; redirection-capturable) for
    the acceptance before/after evidence.
  - **Config.** `tauri.conf.json`: `bundle.createUpdaterArtifacts = true` + a `plugins.updater` block
    (the real minisign **public** key `27E1C773C0BDF81E`, the GitHub-Releases `latest.json` endpoint,
    Windows `installMode: "passive"`). No CSP change (the fetch is Rust-side reqwest, not the webview)
    and **no updater capability** (the flow is driven by our own commands, not the plugin's JS surface —
    keeps typed-IPC-only and gives PR3's tray the same Rust entry point). `.gitignore` gains `*.key` /
    `*.key.pub` as a belt-and-braces guard (the private key lives outside the repo).
  - **UI (all quiet + tokens-only, five states per `UI_REFERENCE §4`).** `commands.ts` / `queryKeys.ts`
    / `queries.ts` (`useUpdateStatus`) / `mutations.ts` (`useCheckForUpdates`, `useRestartToApplyUpdate`,
    no toasts) / `events.ts` + `useLiveEvents.ts` (`update_status_changed` → cache mirror, no toast).
    New `components/shell/UpdateIndicator.tsx` (NavRail footer: a presence dot **only** while an update
    exists — never a count — plus the quiet manual "Check for updates" control) and
    `components/domain/AppPanel.tsx` (Settings · **App** section: update status line + check/restart +
    version/repo link; self-contained like `ApiPanel`, PR3 adds run-at-startup/close-to-tray, PR5 owns
    final placement). `UI_REFERENCE §3`/`§5` touched to name the footer manual-check control (the
    "footer button now" decision, gap #99).
  - **Release pipeline.** New `scripts/make-latest-json.mjs` (emits the signed `latest.json`; hard-fails
    on a tag/version mismatch or a missing `.sig`, so an unsigned build can never yield a manifest) and
    `.github/workflows/release.yml` (tag `v*` → windows-latest build + sign with
    `TAURI_SIGNING_PRIVATE_KEY` → manifest → **draft** release with installer + `.sig` + `latest.json`;
    `workflow_dispatch` dry-run uploads artifacts). Maintainer writes notes + publishes (repo culture).
  - **Docs.** `docs/TESTING.md` auto-update runbook (positive + negative signature test + the
    publish-as-full-release reminder); `CHANGELOG.md` `[Unreleased]`; `specs/07` row #96 (built) + the
    updater-key custody record.
- **Why:** `docs/0.3.2.md` §3 PR2 + `03 §11b` — auto-update must land before 0.4.0 so the sessions
  release reaches installed copies. Driving it Rust-side keeps the UI on typed IPC only and lets PR3's
  tray reuse the same commands.
- **Verification:** `cd ui && npm run lint` (clean) `&& npm run build` (clean); `node scripts/stage-mcp.mjs`;
  `cargo fmt --all -- --check` (clean) · `cargo clippy --workspace --all-targets -- -D warnings` (clean) ·
  `cargo build --workspace` (ok) · `cargo test --workspace` (all green, 0 failed) ·
  `git diff --exit-code -- ui/src/bindings` (clean; new `UpdateStatus.ts` committed). Live E2E
  (signed installer detect → background download → signature-verify → install-on-restart, plus the
  tampered-manifest rejection) per the `docs/TESTING.md` runbook — evidence quoted on the PR.
- **Manual gate (D2 — RELEASE BLOCKER):** the production minisign keypair was generated
  (fingerprint `27E1C773C0BDF81E`, public key in `tauri.conf.json`); the maintainer must set the CI
  secrets `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`) and make the **offline backup** before tagging
  `v0.3.2` (`specs/07`). Losing the private key strands every installed copy on manual downloads.
  Windows code signing (Authenticode) is **not** this PR — that `07` row stays open.

---

## 2026-07-05 — 0.3.2 PR1: specs contract (P7.2 product-shell mini-arc; specs-only)

- **Change:** Normalized the 0.3.2 roadmap (`docs/0.3.2.md`, decisions D1–D12) into the specs —
  no code, no schema, no UI. (a) `specs/04`: `docs/0.3.2.md` in the §1 mandatory reading order
  (hard constraints: zero DB schema migrations D10, new settings only where a PR names them,
  presentation-first D7, structural-only D12); a 0.3.2 row in §2; a §3 build-order bullet for
  PR1→PR6 (Rust lane PR2→PR3 sequential ∥ UI lane PR4; PR5 after both; PR4 reproduce-first + the
  WebView2 stop condition; D5 reviewed-import for PR3); the §4 network guardrail extended to admit
  the signed GitHub-Releases update check. (b) `specs/03`: new **§7d** (tray icon + menu,
  close-to-tray, single-instance codified, clean quit via §6 — D3/D4, no new chords) and **§11b**
  (updater: plugin, minisign manifest, signature-rejection negative requirement, passive D1 UX,
  key custody D2, genesis note minisign ≠ Authenticode); §8 **0.3.2 lifecycle keys**
  (`app.close_to_tray` true / `app.run_at_startup` false — names flagged as a PR1 proposal) + the
  two dead-setting retirements with load tolerance (D8) + the `uia_suppress_during_input_ms`
  cross-ref fix. (c) `specs/UI_REFERENCE.md`: §3 two-tier Settings IA (D6/D7), App section, tray +
  updater blocks (+ tree lines); §4 rows Tray / Updater indicator / Settings·App; §5
  `UpdateIndicator` + `TrayMenu`; §8 **shell layout contract** (D9, acceptance-grade, binds all
  future UI work); §10 item 9. (d) `specs/07`: rows #96/#97/#100/#83 pointed at PR2/PR3/PR5/PR5;
  new #102 (#88 fold-forward, D11) / #103 (settings search deferred, D6) / #104 (visual-refresh
  possibility, D12); the **updater-key custody manual step** (D2, release blocker).
  (e) `CLAUDE.md`/`AGENTS.md`: current-state names the active 0.3.2 arc; the no-cloud hard rule
  admits the signed update check. (f) `CHANGELOG.md`: Docs entry under `[Unreleased]`.
- **Why:** `docs/0.3.2.md` §3 PR1 — every later PR must be implementable from the specs alone
  without reopening the roadmap (`04 §1`/`§2`); the guardrail edits prevent a false
  spec-contradiction stop when PR2 adds the updater's outbound HTTPS check.
- **Verification:** `git diff --name-only main` → only `.md` files (verbatim list on the PR); the
  D1–D12 → spec-location traceability table pasted on the PR. No build/test impact possible (docs
  only); CI runs the full suite on the PR regardless.
- **Review-response addendum (second commit on the PR):** five automated-review findings applied,
  all specs-only. `07` #97's legacy "D6" (0.3.1's pull-based numbering) relabelled to **D4** to match
  0.3.2's namespace (D6 is now two-tier IA), removing a real "spec contradictory → STOP" trap. `02`
  brought into scope (**10 files now, not 9**): §8 Status said "No active arc" and named only the
  lifecycle half — fixed to name the active arc with lifecycle **and** interface + zero-schema, and
  annotated the §5 "Later" auto-update mention (no new `§5d`). `UI_REFERENCE §8` shell matrix widened
  from "five routes" to all six content routes (naming **Moment**). Two `03` formatting nits (§7d
  run-at-startup gets its own `(D3).` clause; §8 lifecycle-keys parenthetical split into sentences).

---

> Pre-0.2.x (v0.1.0) history → `specs/archive/08_CHANGELOG_AI.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/08_CHANGELOG_AI.v0.2.x.md`.
> Shipped 0.3.0 history (PR1–PR9 + bridge fixes) → `specs/archive/08_CHANGELOG_AI.v0.3.0.md`.
> Shipped 0.3.1 history (post-0.3.0 bridge fixes + PR1–PR4) →
> `specs/archive/08_CHANGELOG_AI.v0.3.1.md`.
> Live file holds only the current arc — empty until the next arc begins.
