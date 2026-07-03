# TODO — active deferred work

> Pending, deliberately-deferred work that is **planned and specced** but not yet implemented.
> Each entry is self-contained enough to pick up cold (e.g. after a context reset). Closed items
> move to `07_KNOWN_GAPS.md` / `08_CHANGELOG_AI.md` once done. Newest first.

---

## TODO-3 — Settings-level hotkey-conflict check for the mark/overlay chords (deferred from PR #75, 0.3.0 PR6)

**Status:** OPEN. Deferred **by decision**, not by time. Reviewer suggestion from PR #75
(claude hand review). Low value given the existing OS-level guard; recorded so the deferral is
deliberate.

### The suggestion
`sanitize_settings` (`crates/kernel/src/settings.rs`) accepts `overlay_hotkey` and `marks_hotkey`
set to the **same chord** silently. A settings-layer cross-check could reject/flag the collision
earlier and more cheaply than an OS round-trip.

### Why it's deferred (the trade-offs)
- **The conflict is already caught, loudly.** When two chords collide, the second
  `global_shortcut().on_shortcut(...)` registration fails at OS-registration time and surfaces the
  D6 warning toast + a `failed` `HotkeyStatus` (`src-tauri/src/overlay.rs`
  `register_*_hotkey` → `emit_hotkey_warning`). The user sees it immediately; nothing is silently
  dropped. This is the same path every other unavailable-chord case flows through.
- **Consistency.** Every hotkey availability decision currently lives at registration time, keyed by
  the actual OS response — the only source of truth for "is this chord free" (a chord can also
  collide with a *third-party* global hotkey, which a settings-layer app-internal check can't know).
  Adding a partial app-internal check duplicates that logic while still not being authoritative.
- **Not hot / not a correctness bug.** Settings save is user-initiated and rare; the collision is
  recoverable (pick another chord) and observable.

### If ever implemented — do it without fragmenting the source of truth
- Treat a settings-layer check as a *fast-path UX hint* (e.g. inline field warning "same as Flow
  overlay"), not as the authority — keep the OS-registration path as the real gate so third-party
  collisions still surface.
- Mirror the check in both the Rust `sanitize_settings` and the UI `sanitizeSettings`
  (`ui/src/routes/Settings.tsx`) if it's meant to block save, and add a round-trip test.

### Also considered, rejected (not a TODO)
- **Done/Dismiss in `IntentionsStrip` share one resolve op** (claude review): this is the documented
  backend design — dismiss = resolve-with-no-action, the same `resolve_mark` call
  (`crates/store/src/marks.rs`). Two labels, one op, by intent. No change warranted.

### Files
- `crates/kernel/src/settings.rs` (`sanitize_settings`) · `src-tauri/src/overlay.rs` (the live
  registration gate that already handles this) · `ui/src/routes/Settings.tsx` (`sanitizeSettings`,
  if the check should also live client-side).

### References
- PR #75 review (claude hand review, finding #3) · `07_KNOWN_GAPS.md` (0.3.0 PR6 decisions) ·
  D6 loud-conflict decision.

---

## TODO-2 — Bulk span-merge for the purged-span backfill (deferred from `fix/degrade-to-text-db-growth`, `07` #73a)

**Status:** OPEN. Deferred **by decision**, not by time. Reviewer suggestion from PR #67
(gemini-code-assist, flagged "high" against its N+1 guideline). Low value here; recorded so the
deferral is deliberate.

### The suggestion
Replace the per-frame `store.merge_frame_spans_to_lines(id)` calls in the one-time backfill
(`src-tauri/src/lib.rs::merge_purged_spans_once`) with a single bulk method
`merge_batch_frame_spans_to_lines(frame_ids)` that fetches all spans for a 256-id batch with one
`WHERE frame_id IN (…)` query and merges them inside one transaction — turning N per-frame
round-trips into ~1 per batch.

### Why it's deferred (the trade-offs the guideline misses for this codebase)
- **Embedded SQLite, no network hop.** The N+1 "round-trip" cost the guideline targets is a
  networked-DB concern. Here every query is an in-process `rusqlite` call against a local file; the
  per-call overhead is a prepared-statement step, not a network round-trip.
- **Neither path is hot.** `merge_purged_spans_once` is a one-time backfill that drains the
  pre-existing purged backlog once, then watermarks off forever (`maintenance.purged_spans_merged`).
  The inline degrade is one frame per retention-sweep iteration (hourly). No steady-state cost.
- **Per-frame transactions are intentional.** They keep each write-lock window tiny so the sweep /
  backfill never blocks live capture writes, and — critically — they give **per-frame failure
  isolation**: the current `clean_drain` logic skips a single busy frame and retries just that frame
  next launch. A single `IN`-clause transaction rolls back the **whole** 256-batch on one busy
  frame; since the same frame poisons the batch on every retry, the backfill could **never
  converge**. A naive bulk rewrite is strictly worse resilience.

### If ever implemented — do it without regressing resilience
- Bound the batch inside one transaction, but keep per-frame error isolation: a failed
  `replace_text_spans` for one id must not abort the others' commits (e.g. `SAVEPOINT` per id, or
  collect failures and still commit the good ones while withholding the watermark). Verify a
  poisoned frame can't strand the whole batch's progress.
- Keep the retention sweep's atomic `degrade_frame_to_text` per-frame regardless — it interleaves
  with per-frame file deletion + the `image_purged` flag and can't be batched.

### Files
- `crates/store/src/records.rs` (new `merge_batch_frame_spans_to_lines`; `read_text_spans` already
  exists — add a batched read or loop reads inside one tx)
- `src-tauri/src/lib.rs::merge_purged_spans_once` (call the batch method per 256-id page)

### References
- PR #67 review threads (gemini-code-assist on `records.rs` `merge_frame_spans_to_lines` and on
  `lib.rs` `merge_purged_spans_once`) · `07_KNOWN_GAPS.md` #73a · `08_CHANGELOG_AI.md` (2026-07-01).

---

## TODO-1 — UIA `FindAllBuildCache` cached single-round-trip walk (deferred from `fix/uia-chromium-hang`, `07` #71)

**Status: ✅ RESOLVED (2026-07-01, `fix/uia-findall-buildcache`).** Live verification rejected the
`FindAllBuildCache(Subtree)` single-round-trip design below (one uninterruptible ~1.4 s fetch overran
the budget) **and** a `FindAllBuildCache(Children)` BFS (a single wide-node fetch overran on VS Code-
scale trees). Shipped instead: a granular DFS that keeps the old walker navigation but batches each
node's property reads into one **`BuildUpdatedCache`** call (~2.5× fewer COM calls, same boundedness).
See `07` #71 / `05` / `08`. The original single-round-trip write-up below is kept for history.

**(historical)** Planned, API-verified, not-yet-implemented single-round-trip design:

### Why it's deferred (read this first)
The Chromium/Electron UIA-hang fix shipped in PR #48 as a mitigation: don't walk on scroll/click,
an in-flight guard + bounded channel kill the walk backlog, `ControlViewWalker` instead of raw view,
and the live `TextPattern` read is gated to Document/Edit/Text controls and capped. **That already
removes the hang.** This remaining item is the *efficiency/coverage* lever from the original plan: it
makes each (idle/timer/foreground) walk far cheaper on big trees.

It was held back **on purpose**, not for time:
- The walk path (`crates/uia/src/worker.rs::read_foreground`) is exercised **only** by the
  `#[ignore]`d live test `uia_provider_spawns_and_recognizes_foreground` (needs a real desktop
  session) — CI never runs it.
- A subtle COM/cache bug (wrong VARIANT type, wrong scope, `_None` mode dropping TextPattern, a
  cached getter returning empty) would make UIA **silently** return below `min_text_chars` →
  always-thin-yield → OCR fallback for every frame. **No CI signal, no crash, no hang** — just UIA
  quietly doing nothing. That is exactly the failure class our rules say not to ship unobserved.
- So it must be implemented **with live verification**: run the ignored test + `npm run tauri dev`
  and confirm UIA still yields real text (`frame_text.primary_source='uia'`) before trusting it.

### Goal
Replace the per-node **live** `Current*` reads (≈5–10 cross-process COM calls per node, up to
`capture.uia_max_nodes` nodes) with **one** `FindAllBuildCache` round-trip that pre-fetches the
needed properties into a client-side cache, then read `Cached*` getters in-process. Turns ≈N×6
cross-process calls into ≈1. TextPattern stays live but is already gated/capped (keep that).

### Current state to build on (already in `crates/uia/src/worker.rs`)
- `read_foreground` does an iterative DFS with `ControlViewWalker()` (or RawView when
  `budget.control_view == false`), bounded by `budget.max_nodes`, `MAX_DEPTH`, `MAX_SPANS`,
  `MAX_STACK`, and the `budget.latency_ms` soft deadline.
- Per node it reads `CurrentControlType` / `CurrentIsPassword` / `CurrentIsOffscreen`, then
  `extract_text(&elem, allow_textpattern)` (cached-free), then `CurrentBoundingRectangle`.
- `extract_text` priority ladder: live `TextPattern` visible ranges (only when
  `classify::control_type_wants_textpattern` and under `budget.max_textpattern_calls`) →
  `ValuePattern` value → `Name`.
- `UiaBudget` already carries `max_nodes`, `max_textpattern_calls`, `control_view`; settings
  `capture.uia_*` already plumbed (`traits`, `kernel::settings`, Settings UI, ts-rs binding).
- `IUIAutomation2::SetConnectionTimeout`/`SetTransactionTimeout(budget.latency_ms)` already set —
  this bounds the single big `FindAllBuildCache` call (the structural fix for "deadline can't
  interrupt one call": with one call + a handful of capped TextPattern calls, that stops mattering).

### Windows-rs API surface — VERIFIED present in the pinned `windows 0.62.2`
(workspace `Cargo.toml` pins `windows = "0.62"`; `Cargo.lock` → `windows 0.62.2`; checked in
`~/.cargo/.../windows-0.62.2/src/Windows/Win32/UI/Accessibility/mod.rs`)
- `IUIAutomation::CreateCacheRequest() -> IUIAutomationCacheRequest`
- `IUIAutomation::CreateTrueCondition() -> IUIAutomationCondition`
- `IUIAutomation::CreatePropertyCondition(UIA_PROPERTY_ID, &VARIANT) -> IUIAutomationCondition`
- `IUIAutomation::CreateAndCondition(...)`; `ControlViewWalker()/ContentViewWalker()/RawViewWalker()`
- `IUIAutomationCacheRequest::{ AddProperty(UIA_PROPERTY_ID), AddPattern(UIA_PATTERN_ID),
  SetTreeScope(TreeScope), SetAutomationElementMode(AutomationElementMode) }`
- `AutomationElementMode_None = 0`, `AutomationElementMode_Full = 1`
- `IUIAutomationElement::{ FindAllBuildCache(scope, condition, cacheRequest)
  -> IUIAutomationElementArray, FindFirstBuildCache, BuildUpdatedCache, GetCachedChildren() }`
- cached getters: `CachedControlType()->UIA_CONTROLTYPE_ID`, `CachedName()->BSTR`,
  `CachedIsPassword()->BOOL`, `CachedIsOffscreen()->BOOL`, `CachedBoundingRectangle()->RECT`,
  `GetCachedPropertyValue(id)->VARIANT`, `GetCachedPattern(id)->IUnknown`,
  `GetCachedPatternAs::<T>(id)->T`
- `IUIAutomationElementArray::{ Length()->i32, GetElement(i)->IUIAutomationElement }`
- Property IDs: `ControlType=30003`, `Name=30005`, `IsPassword=30019`, `IsOffscreen=30022`,
  `BoundingRectangle=30001`, `ValueValue=30045`, `IsTextPatternAvailable=30040`,
  `IsValuePatternAvailable=30043`, `IsControlElement=30016`, `IsContentElement=30017`
- Pattern IDs: `Text=10014`, `Value=10002`
- `TreeScope_Subtree=7`, `TreeScope_Children=2`, `TreeScope_Element=1`
- `IUIAutomationTextPattern::GetVisibleRanges()->IUIAutomationTextRangeArray`

### Cacheability constraints (design only around these)
- **Cacheable in one round-trip:** ControlType, Name, IsPassword, IsOffscreen, BoundingRectangle,
  ValuePattern value (read as the property `GetCachedPropertyValue(UIA_ValueValuePropertyId=30045)`),
  IsTextPatternAvailable, IsControlElement.
- **NOT cacheable:** `IUIAutomationTextPattern::GetVisibleRanges()` + `IUIAutomationTextRange::GetText()`.
  Text ranges are live; even a cached TextPattern object re-enters the provider on the range calls,
  and `GetCachedPatternAs::<IUIAutomationTextPattern>` only returns a usable object under
  `AutomationElementMode_Full` (a `_None`-mode element has no live backing to query ranges from).
  → keep the existing gate (`control_type_wants_textpattern` + `max_textpattern_calls`).

### Implementation steps (in `crates/uia/src/worker.rs`)
1. **`build_cache_request(&IUIAutomation) -> Result<IUIAutomationCacheRequest>`:** `CreateCacheRequest`;
   `AddProperty` for ControlType(30003), Name(30005), IsPassword(30019), IsOffscreen(30022),
   BoundingRectangle(30001), ValueValue(30045), IsTextPatternAvailable(30040), IsControlElement(30016);
   `SetTreeScope(TreeScope_Subtree=7)`; `SetAutomationElementMode(AutomationElementMode_Full)`.
   → **Decision (from the plan, user-confirmed):** `_Full`, to keep document/editor TextPattern body
   text — the highest-value UIA text and the reason UIA is default-on.
2. **One batched fetch instead of the manual DFS:**
   - PREFERRED: a control-element condition + one call —
     `automation.CreatePropertyCondition(UIA_IsControlElementPropertyId=30016, VARIANT(bool true))`
     then `root.FindAllBuildCache(TreeScope_Subtree, &cond, &cache)`.
     **⚠ The one fiddly bit:** constructing a `VARIANT` holding a `VARIANT_BOOL` true in
     windows 0.62.2. Verify the ergonomics (`VARIANT::from(true)` / explicit `VARIANT_BOOL`); if
     awkward, use the fallback.
   - FALLBACK (still ONE round-trip, no VARIANT): `CreateTrueCondition()` +
     `FindAllBuildCache(TreeScope_Subtree, &true, &cache)`, then skip elements whose cached
     `IsControlElement` is false in the in-process loop. Bigger array, but the cross-process cost
     (the actual hang cause) is paid exactly once.
   - When `budget.control_view == false`, keep the legacy raw walk (or widen the condition).
3. **Iterate the returned `IUIAutomationElementArray`** (`Length`/`GetElement`), bounded by
   `budget.max_nodes` + the soft deadline, reading **cached** getters only
   (`CachedControlType`/`CachedIsPassword`/`CachedIsOffscreen`/`CachedName`/`CachedBoundingRectangle`
   + `GetCachedPropertyValue` for ValueValue/IsTextPatternAvailable). Reuse `classify::should_emit`,
   `within_target`, `geometry::normalize_screen_rect`, `classify::split_words` unchanged.
4. **`extract_text`:** read ValueValue + Name from the **cache**; keep the live `TextPattern` branch
   exactly as today but gated by cached `IsTextPatternAvailable` AND
   `control_type_wants_textpattern` AND the `max_textpattern_calls` cap.
5. **Telemetry:** keep the per-walk `debug!(nodes, spans, elapsed_ms)` + rate-limited over-budget
   `warn!`; consider logging `cache_elems` (array length) too.

### Testable units (TDD) + acceptance
- Pure/CI: any new classifier helpers (none strictly needed; `should_emit` /
  `control_type_wants_textpattern` already covered).
- **Real acceptance gate (must run on a desktop):**
  - `cargo test -p uia -- --ignored` — extend `uia_provider_spawns_and_recognizes_foreground` to
    assert the walk returns within the hard timeout and yields ≥ some chars; optionally log node/cache
    counts.
  - `npm run tauri dev`, open a long Chromium/Electron page (e.g. the qBittorrent web UI grid),
    scroll: target stays responsive; DB shows `frame_text.primary_source='uia'` for idle/timer frames
    and node count / elapsed dropping vs the manual walk.
- Full CI gates per `CLAUDE.md` (UI build first, then fmt/clippy -D warnings/build/test, binding
  guard clean — though this change likely touches no `Settings`, so bindings stay clean).

### Files
- `crates/uia/src/worker.rs` (primary: `read_foreground`, `extract_text`, new `build_cache_request`)
- `crates/uia/src/lib.rs` (only if the budget/threading shape needs adjusting — probably not)
- No schema change; settings already exist.

### References
- `07_KNOWN_GAPS.md` #71 (the hang + what shipped vs deferred) · `08_CHANGELOG_AI.md` (the 3 shipped
  commits) · PR #48 (`fix/uia-chromium-hang`) · the full design write-up that produced this lives in
  the session plan file `ultracode-bug-when-using-purring-turtle*.md`.
