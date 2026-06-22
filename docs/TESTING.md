# Testing — quick guide

Run from the repo root. Each command stands alone. Green = good.

## ✅ The one command

```sh
cargo test --workspace
```

That runs every test. **0 failed = pass.** (GPU/model tests are skipped automatically — that's normal.)

## 🔁 Before you push (4 gates)

Run these four. All must be clean:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd ui && npm run build && cd ..
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
