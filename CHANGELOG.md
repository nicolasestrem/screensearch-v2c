# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Added — P0 Scaffold (2026-06-21)
- Cargo workspace with a modular kernel layout (`03 §2`): `traits` (contracts +
  domain/jobs/IPC types) and skeleton crates `kernel`, `store`, `capture`, `ocr`,
  `embeddings`, `inference`.
- `src-tauri` — Tauri 2 desktop shell (composition root) with typed `ping` and
  `get_readiness` commands.
- React 18 + TypeScript + Vite UI skeleton (`ui/`) with a P0 typed-IPC smoke screen
  and an ESLint flat config enforcing the Rules-of-Hooks gate at error level.
- Typed UI↔core contract generated with `ts-rs` into `ui/src/bindings/`, with a
  regression guard keeping 64-bit ids as TS `number` (Tauri JSON wire).
- `doctor` environment smoke-check (`cargo run -p doctor`) for WebView2 / Vulkan /
  llama-server.
- GitHub Actions CI (`.github/workflows/ci.yml`, windows-latest): UI lint/build,
  `cargo fmt`/`clippy -D warnings`/`build`/`test`, and a ts-rs binding-drift guard.
- Generated application icons from `assets/icon-source.png`.

### Fixed (post-P0 review)
- `doctor`: load `vulkan-1.dll` from an absolute System32 path instead of by bare
  name, avoiding DLL search-order hijacking.
- UI: issue the independent `ping` / `get_readiness` IPC calls in parallel.
- CI: Claude review now actually posts — added the `--comment` flag (the skill
  produces a review but only posts with it) and granted the workflows write
  permissions (posts were silently denied before); `concurrency` added; actions
  bumped to Node-24 majors.

_Scaffold only — no capture, storage, embeddings, or inference yet (P1–P4)._
