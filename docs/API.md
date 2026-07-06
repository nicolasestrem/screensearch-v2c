# ScreenSearch Local API (v1)

The **local HTTP API** exposes ScreenSearch's search, ask, frames, where-was-i, and marks
over `127.0.0.1` for local scripts and agents. It is **opt-in and off by default** (0.3.0
PR7; spec `03 §7c`).

> **Threat model — read this before enabling.** *Any local process holding the token can
> read your entire screen history — enabling this is an explicit trust decision.* The API
> binds `127.0.0.1` only (hard-coded, never your network) and requires a bearer token on
> every request, but it does not sandbox which local process presents that token.

## Enabling

Settings → **Local API** → toggle on. This:

- binds `127.0.0.1:<port>` (default **43210**, configurable; the bind address is not);
- generates a **bearer token** on first enable (shown in Settings — reveal / copy /
  regenerate; regenerating takes effect immediately, no restart);
- if the port is already in use, does **not** start — Settings shows a warning + an inline
  "pick another port" retry.

## Authentication

Every request to every route (health included) must send:

```
Authorization: Bearer <token>
```

A missing or wrong token returns **401**. The token is compared in constant time.

```bash
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:43210/v1/health
```

## Errors

Every non-2xx response is a JSON body — including malformed query strings, bodies, and path
segments, which are mapped to `400 bad_request` rather than a framework plaintext rejection, so
clients can parse errors uniformly:

```json
{ "error": "not_found", "message": "frame 42 not found" }
```

| Status | `error`         | When |
|--------|-----------------|------|
| 400    | `bad_request`   | Malformed params/body (e.g. missing `q`, `from`/`to` not paired, `from` > `to`, `format` ≠ `json`, both `frame_id` and `now`). |
| 401    | `unauthorized`  | Missing or wrong bearer token. |
| 404    | `not_found`     | Unknown frame or mark — or an unknown endpoint path. |
| 404    | `image_purged`  | The frame exists but its screenshot was retention-purged (text is preserved). |
| 503    | `unavailable`   | A dependency is unavailable (answer model not loaded; capture off for `POST /v1/marks {"now":true}`). |
| 500    | `internal`      | Unexpected store failure. |

## Endpoints

All paths are under `http://127.0.0.1:<port>`.

### `GET /v1/health`

```json
{ "version": "0.3.2", "uptime_secs": 128, "capturing": true }
```

### `GET /v1/search`

Query params: `q` (required), `from`, `to` (unix ms, half-open `[from, to)` — provide both
or neither), `limit` (default 20, clamped `1..=100`), `include_chrome` (default = your
`text.include_chrome_default`).

```bash
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:43210/v1/search?q=invoice&limit=10"
```

Returns an array of `SearchHit` (`frame_id`, `captured_at`, `snippet`, `score`,
`image_path`, `image_purged`, `app_hint`) — the same shape and ranking as the UI.

### `POST /v1/ask`

Body: `{ "query": "...", "top_k"?: number, "thinking"?: bool, "max_tokens"?: number }`.
Returns a **Server-Sent Events** stream (`Content-Type: text/event-stream`); each `data:`
line is a JSON `AnswerDelta`:

```
data: {"type":"thinking","text":"…"}
data: {"type":"token","text":"The invoice total was …"}
data: {"type":"citation","frame_id":41}
data: {"type":"done"}
```

`{"type":"error","message":"…"}` may replace the terminal `done`. A keep-alive comment is
sent every 15 s. **Disconnecting the client cancels generation** — a dropped connection
stops the model rather than leaving it generating into a closed socket. Returns **503** if
no answer model is loaded.

```bash
curl -N -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"query":"what did I read about rustls?"}' \
  http://127.0.0.1:43210/v1/ask
```

### `GET /v1/frames/{id}`

Default: the frame's metadata + text (`FrameDetail`). `?image=1`: the stored screenshot
bytes (`image/webp`, or `image/jpeg` for pre-WebP frames). Unknown id → 404; purged image
→ 404 `image_purged`.

```bash
curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:43210/v1/frames/41"
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:43210/v1/frames/41?image=1" --output frame.webp
```

### `GET /v1/context/where-was-i`

Returns the last sustained context before your current detour (`ResumeContext`), or `null`
when nothing qualifies.

### Marks — the only write surface

- `GET /v1/marks` → all marks, unresolved first then newest-first (`Mark[]`).
- `POST /v1/marks` → create one. Body sets **exactly one** of:
  - `{ "frame_id": <id>, "note"?: "..." }` — mark an existing frame;
  - `{ "now": true, "note"?: "..." }` — capture the current screen past the diff gate,
    then mark it (503 if capture is off / a privacy gate denies).

  Returns `201 { "mark_id": <id> }`.
- `POST /v1/marks/{id}/resolve` → resolve (or dismiss) a mark → `200 { "resolved": true }`.
  Idempotent; unknown id → 404.

### `GET /v1/export`

Query params: `from`, `to` (unix ms, optional window), `format` (only `json` in v1; anything
else → 400). Streams a single JSON document:

```json
{
  "schema": "screensearch.export.v1",
  "exported_at": 1720000000000,
  "from": null,
  "to": null,
  "frames": [
    { "frame_id": 1, "captured_at": 1719999990000, "app_hint": "VS Code",
      "window_title": "main.rs", "browser_url": null, "activity_type": null,
      "content_text": "…" }
  ],
  "marks": [
    { "mark_id": 1, "frame_id": 1, "created_at": 1719999991000, "note": null,
      "resolved_at": null }
  ]
}
```

Frames carry **content text only** — never raw OCR, never image bytes (D12). The response
is streamed (paged internally), so exporting months of history stays memory-flat. If a store
error occurs mid-stream, the body is truncated and the connection closes (an honest partial
failure). The Settings **Export…** button calls the same code path to write a file to your
Downloads folder, so export works with the API disabled.

## Versioning

This is **v1**, read-only except the marks writes above. The MCP server (`docs/MCP.md`,
0.3.0 PR8) wraps this same HTTP surface for MCP clients.
