# ScreenSearch V2c - 0.2.0 PR8 Audit (2026-06-26)

Branch: `codex/0.2.0-pr8-audit`

Evidence directory: `.playwright-mcp/pr8-2026-06-26/`

## Verdict

**PR8 passes audit for the 0.2.0 release scope.** The parallel model downloader and the follow-up
hardening commit are present on current `main`, confined to the inference downloader surface, and
verified by mocked regression tests plus a real `npm run tauri dev` UI run against a reset app-data
state.

No PR8 release blocker was found. The existing PR3 static/app-chrome retrieval failure remains a
separate 0.2.0 release blocker and is not changed by this PR8 audit.

Accepted follow-up from this audit: PR8 now rejects stale complete and partly-complete manifests
when a `.part` file is brand-new, but an unusual external cleanup case can still leave an existing
header-matching `.parts` bitmap beside an existing-but-truncated `.part`; `open_preallocated` extends
that file back to full length and the stale done bits could still be trusted. This is recorded in
`specs/07_KNOWN_GAPS.md` as a narrow non-blocking hardening gap.

## Scope Checked

- `crates/inference/src/download.rs`: parallel `Range` requests, Hugging Face per-request resolve,
  resume bitmap, aggregate progress, stall handling, clean-layout publish, range-less fallback, and
  sha256 integrity.
- PR8 follow-up hardening: fresh `.part` stale-manifest reset, network-error retry, unreadable
  manifest handling, coalesced writes, and accurate terminal chunk errors.
- Public surface drift: no schema migration, IPC binding, Tauri command, or public API change was
  introduced by PR8. The only expected dependency/API additions are `sha2` and the downloader env
  overrides `SSV2C_DOWNLOAD_CONNECTIONS` / `SSV2C_DOWNLOAD_CHUNK_SIZE`.
- PR3 static-chrome failure is cross-referenced only as release status, per the audit plan.

## Static Findings

- `fetch_one` preserves the existing installed-file and cache-copy fast paths before entering the
  chunked path.
- `probe_range` inspects the stable `resolve` response without following the redirect, so
  `X-Linked-ETag` and `X-Linked-Size` are read from the Hugging Face resolve layer rather than the
  CDN blob.
- `range_plan` requires known length and range support before using chunked download. The fallback is
  the existing hf-hub single-stream path for range-less servers.
- `chunked_download` pre-allocates one `.part`, writes chunks with positioned writes, fsyncs the data
  before setting bitmap bits, and atomically renames the verified file into the clean model layout.
- `fetch_chunk` re-resolves the Hugging Face `resolve` URL per chunk while preserving `Range`,
  retries transient HTTP and transport errors, and rejects a `200 OK` response to a `Range` request
  as "server ignored Range".
- Progress uses one aggregate downloaded-byte counter, so concurrent chunks still feed the same UI
  progress and stall watchdog.
- Sha256 integrity is tied to the trusted LFS `X-Linked-ETag`; bare CDN/Xet `ETag` is ignored.

## Targeted Scenarios

| Scenario | Result | Evidence |
|---|---|---|
| Parallel byte-identical assembly | Pass | `download::tests::chunked_download_assembles_byte_identical_file`; live GGUF/mmproj final lengths matched expected file sizes. |
| Resume skips completed chunks | Pass | `download::tests::resume_skips_already_completed_chunks`; live Vision Beta resumed from 73/170 completed chunks and restarted at 86%, not 0%. |
| Stuck chunk fails fast | Pass | `download::tests::chunked_download_fails_fast_on_stuck_chunk`. |
| Server ignoring `Range` is rejected | Pass | `download::tests::chunked_download_errors_when_server_ignores_range`. |
| Transient 403/network error retries | Pass | `download::tests::chunk_retries_transient_403_then_succeeds`; `download::tests::exhausted_transient_is_not_reported_as_ignored_range`; `download::tests::manifest_load_or_init_distinguishes_missing_valid_and_mismatched`. |
| Sha256 accepts correct `X-Linked-ETag` and rejects bad hash | Pass | `download::tests::lfs_sha256_trusts_only_x_linked_etag_not_cdn_etag`; `download::tests::integrity_accepts_matching_sha256_and_rejects_a_wrong_one`. |
| Fresh `.part` discards stale complete or partial manifest | Pass | `download::tests::fresh_part_discards_stale_all_done_manifest`; `download::tests::fresh_part_discards_stale_partial_manifest`. |
| Range-less fallback path exists | Pass by static/unit review | `range_plan_requires_ranges_and_known_size`; fallback remains `download_with_lock_retry`. Gap #46 still applies only to this fallback path. |

## Live Dev-Exe Audit

Command used: `npm run tauri dev`

Executable observed through Computer Use:
`C:\Users\nicol\Documents\GitHub\screensearch-v2c\target\debug\screensearch.exe`

App data was reset before the run as requested. The app created
`C:\Users\nicol\AppData\Roaming\app.screensearchv2c.desktop` and repopulated models/settings/DB
during the audit.

### Answer Lane Default

From Settings -> Inference engine, `LOAD ANSWER MODEL` started a first-run answer model download.
The StatusRail/UI showed live progress, including:

```text
Downloading Ministral-3B-Reasoning-2512-GGUF... 11% · 218 MB / 2.0 GB
```

The download completed and the UI reported `Loaded` for the answer model:

```text
C:\Users\nicol\AppData\Roaming\app.screensearchv2c.desktop\models\answer\default\Ministral-3B-Reasoning-2512-Q4_K_M.gguf
Length: 2146496256
```

### Vision Quality 8B

Settings -> Inference engine switched Vision model to `Quality`. I started capture, captured a new
frame, opened Moment, and used `TAG WITH VISION` to force the 8B GGUF + mmproj path. The UI showed
progress without freezing, queued/running job state advanced, and the final tag rendered in Moment.

Final files:

```text
C:\Users\nicol\AppData\Roaming\app.screensearchv2c.desktop\models\vision\quality\Qwen3VL-8B-Instruct-Q4_K_M.gguf
Length: 5027784800

C:\Users\nicol\AppData\Roaming\app.screensearchv2c.desktop\models\vision\quality\mmproj-Qwen3VL-8B-Instruct-F16.gguf
Length: 1159029824
```

Post-download, the `.part` and `.parts` files were gone, confirming clean-layout publish and cleanup.
Moment displayed a vision description with model chip `Qwen3VL-8B-Instruct-Q4_K_M.gguf` and
confidence `95%`.

### Interrupted Resume

The normal Quality answer download completed too quickly to interrupt, so I used the user's caveat
for Vision Beta: download/resume was valid to exercise, but tag quality was not treated as a pass
criterion.

The Beta download was started with `SSV2C_DOWNLOAD_CONNECTIONS=1`, then the dev app was stopped
while `.part` and `.parts` existed.

Captured before stop:

```text
Name          : Qwen3.5-9B-Q4_K_M.gguf.part
Length        : 5680522464

Name          : Qwen3.5-9B-Q4_K_M.gguf.parts
Length        : 208

Header  : SSV2CPARTS v1 5680522464 33554432 170
Chunks  : 170
Done    : 73
Pending : 97

Stopped screensearch process
```

After restarting `npm run tauri dev` normally, the UI resumed at `86%` rather than returning to zero.
The download finalized and cleaned up the partial files.

Final Beta files:

```text
C:\Users\nicol\AppData\Roaming\app.screensearchv2c.desktop\models\vision\beta\Qwen3.5-9B-Q4_K_M.gguf
Length: 5680522464

C:\Users\nicol\AppData\Roaming\app.screensearchv2c.desktop\models\vision\beta\mmproj-F16.gguf
Length: 918166080
```

### Cleanup / Orphan Check

After stopping the dev app, no `screensearch.exe` or `llama-server.exe` process remained.

The final state evidence is in:

- `.playwright-mcp/pr8-2026-06-26/live-final-state.txt`
- `.playwright-mcp/pr8-2026-06-26/resume-beta-before-stop.txt`
- `.playwright-mcp/pr8-2026-06-26/tauri-dev.log`
- `.playwright-mcp/pr8-2026-06-26/tauri-dev-resume.log`
- `.playwright-mcp/pr8-2026-06-26/tauri-dev-resume-after.log`

## Verification Outputs

Full raw command outputs are preserved verbatim under `.playwright-mcp/pr8-2026-06-26/`.

### `cd ui && npm ci && npm run lint && npm run build`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-ui-npm-ci-lint-build.txt`

```text
COMMAND: cd ui && npm ci && npm run lint && npm run build

added 347 packages, and audited 348 packages in 4s

151 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
npm warn allow-scripts 1 package has install scripts not yet covered by allowScripts:
npm warn allow-scripts   esbuild@0.25.12 (postinstall: node install.js)
npm warn allow-scripts
npm warn allow-scripts Run `npm approve-scripts --allow-scripts-pending` to review, or `npm approve-scripts <pkg>` to allow.

> screensearch-ui@0.1.0 lint
> eslint .


> screensearch-ui@0.1.0 build
> tsc --noEmit && vite build

vite v6.4.3 building for production...
transforming...
✓ 407 modules transformed.
rendering chunks...
computing gzip size...
✓ built in 1.78s
```

### `cargo fmt --all -- --check`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-cargo-fmt.txt`

```text
COMMAND: cargo fmt --all -- --check
```

### `cargo clippy --workspace --all-targets -- -D warnings`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-cargo-clippy.txt`

```text
COMMAND: cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.90s
```

### `cargo build --workspace`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-cargo-build.txt`

```text
COMMAND: cargo build --workspace
   Compiling inference v0.1.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\inference)
   Compiling screensearch v0.1.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.06s
```

### `cargo test -p inference --lib`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-cargo-test-inference-lib.txt`

```text
COMMAND: cargo test -p inference --lib
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
     Running unittests src\lib.rs (target\debug\deps\inference-572b02788828c698.exe)

running 88 tests
test download::tests::chunked_download_errors_when_server_ignores_range ... ok
test download::tests::fresh_part_discards_stale_all_done_manifest ... ok
test download::tests::resume_skips_already_completed_chunks ... ok
test download::tests::fresh_part_discards_stale_partial_manifest ... ok
test download::tests::chunk_requests_follow_redirect_and_preserve_range ... ok
test download::tests::chunked_download_assembles_byte_identical_file ... ok
test download::tests::integrity_accepts_matching_sha256_and_rejects_a_wrong_one ... ok
test download::tests::chunked_download_fails_fast_on_stuck_chunk ... ok
test download::tests::chunk_retries_transient_403_then_succeeds ... ok
test download::tests::exhausted_transient_is_not_reported_as_ignored_range ... ok

test result: ok. 88 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.11s
```

The inference raw block above is intentionally trimmed to the PR8-owned tests; the full 88-test raw
output is preserved in the evidence file.

### `cargo test --workspace`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-cargo-test-workspace.txt`

The full workspace output is 376 lines and is preserved verbatim in the evidence file. The raw tail
ends with:

```text
   Doc-tests traits

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `git diff --exit-code -- ui/src/bindings`

Raw output file: `.playwright-mcp/pr8-2026-06-26/verify-bindings-diff.txt`

```text
COMMAND: git diff --exit-code -- ui/src/bindings
```

