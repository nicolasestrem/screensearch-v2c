# Testing — quick guide

Run from the repo root. Each command stands alone. Green = good.

## ✅ The one command

```sh
cargo test --workspace
```

That runs every test. **0 failed = pass.** (GPU/model tests are skipped automatically — that's normal.)

## 🔁 Before you push (CI-order gates)

Run these. All must be clean:

```sh
(cd ui && npm ci && npm run lint && npm run build)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
git diff --exit-code -- ui/src/bindings
```

Tip: copy-paste them one at a time. Don't move on until one passes.

## 🩹 If something fails

| You see | Do this |
|---|---|
| `fmt` diff | `cargo fmt --all` (auto-fixes), then re-check |
| `clippy` error | read the `--> file:line`, fix that one spot, re-run |
| a test `FAILED` | scroll up to the test **name** + its `assert` message; that's the clue |
| `npm` error | `cd ui && npm ci` once, then `npm run build` |

## ⏭️ What gets skipped (and that's fine)

Tests marked `ignored` need a GPU, a downloaded model, or hardware. They are **not** run by `cargo test`. Only run these on purpose:

```sh
# real llama-server: downloads models + uses the GPU (slow, big)
cargo test -p inference --test smoke -- --ignored --nocapture
```

## 🎯 Just one crate (faster)

```sh
cargo test -p inference     # the sidecar
cargo test -p kernel        # workers + scheduler
cargo test -p store         # database
```

## 🟢 The no-orphan gate (P4's must-pass)

Proves the sidecar can't outlive the app:

```sh
cargo test -p inference --test no_orphan
```

Want it to be `ok`. If it's not — stop and ask.

## 🧭 Manual PR7 audit

PR7 is a live UI audit over the user's populated app-data store. Run the app with:

```sh
npm run tauri dev
```

Use the existing `%APPDATA%\app.screensearchv2c.desktop` DB. Do not reset or backfill it. Store local
evidence under ignored paths such as `.playwright-mcp/pr7-YYYY-MM-DD/` and, if needed,
`docs/AUDIT_0.2.0_PR7_YYYY-MM-DD.md`; do not put PR7 images in `screenshots/` or commit audit
artifacts.

Audit coverage:

- Recall Search default content text vs. `include app chrome + raw text`.
- Real content terms from the corpus.
- Ask positive grounding and no-evidence behavior. For no-evidence refusals, source-frame tiles may
  appear only as reviewed context labeled `Frames checked`, not as `Cited frames`.
- Daily and Weekly reports, including pass/frame footer metadata.
- A short start/stop capture tick.

The 2026-06-25 run is a local ignored artifact at `docs/AUDIT_0.2.0_PR7_2026-06-25.md`; tracked
summaries live in `CHANGELOG.md` and `specs/05_BUILD_REVIEW.md` / `07_KNOWN_GAPS.md`.
