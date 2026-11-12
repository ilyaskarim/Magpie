---
title: 'Stage b: capture text into SQLite and list it'
type: 'feature'
created: '2026-11-13'
status: 'done'
baseline_commit: '27a7eb7f5fb3b070ab13024ad383f7eab04755cc'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-Magpie-2026-11-13/ARCHITECTURE-SPINE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The history window is empty; nothing is captured or stored yet (CAP-1 text, CAP-3 list).

**Approach:** Restructure `src-tauri/src` into the spine's hexagonal layout, add a clipboard port and a store port with a SQLite adapter, run a capture watcher thread that stores plain-text copies, and render them newest first in the window with keyboard selection.

## Boundaries & Constraints

**Always:** follow AD-1 (ports + fakes), AD-2 (UI refetches on `history-changed`), AD-3 (UUIDv4 id, SHA-256 dedupe key; a re-copy bumps the existing row), AD-4 (i64 ms), AD-9 (commands/DTOs, `src/api.ts` only invoke caller), AD-11 (watcher thread, `Arc<Mutex<>>` store, lock per op), AD-14 (`CoreError` below shell), AD-16 (`app_local_data_dir`, `seq` PK + `id` UNIQUE, FTS with `content_rowid = seq`), AD-17 (selection by id, `window-shown` resets), AD-18 (startup order, launch content not captured, supervised watcher). Schema v1 is created through `PRAGMA user_version` migrations and already includes the FTS5 table and triggers.

**Never:** images, HTML or files (stage e); paste-back (stage c); search UI, pins (stage d); privacy filters, retention, settings (stage f); `unwrap`/`expect` on OS or disk paths outside tests; logging clipboard contents.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| New text | user copies "hello" in another app | new row: kind `text`, preview "hello", source app name, timestamps | N/A |
| Re-copy | "hello" copied again later | same `id`, `last_copied_at` updated, moves to top, still one row | N/A |
| Whitespace-only | copy "   \n" | not stored | N/A |
| Non-text copy | image only on clipboard | not stored in this stage | N/A |
| Long text | 1 MB of text | stored in full; list DTO preview ≤ 200 chars | N/A |
| DB unopenable | data dir not writable | app still launches; window shows readable error | `CoreError` → `AppError` string |
| Watcher panic | adapter panics | thread restarts with backoff | logged without contents |

</frozen-after-approval>

## Code Map

- `src-tauri/src/lib.rs` -- builder/setup; becomes wiring only (AD-18 order).
- `src-tauri/src/{tray,hotkey,window,error}.rs` -- stage a; move under `shell/`, `error.rs` becomes `shell::AppError` wrapping `CoreError`.
- `src-tauri/src/window.rs::show` -- emit `window-shown` here (hotkey/tray/second-instance all call it).
- `src/App.tsx`, `src/App.css` -- stage a empty view; extend, keep Esc → `hide_window`.
- `src-tauri/tauri.conf.json` -- no asset scope needed yet (no images).

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/Cargo.toml` -- add `clipboard-rs = "0.3.5"`, `rusqlite = { version = "=0.42.0", features = ["bundled"] }`, `uuid` (v4), `sha2`, `hex`; macOS: `objc2-app-kit` 0.3 (`NSWorkspace`, `NSRunningApplication`), `objc2-foundation` 0.3; dev: `tempfile`.
- [x] `src-tauri/src/domain/{mod,item,error}.rs` -- `ItemKind`, `Item`, `NewCapture`, `dedupe_key()`, `CoreError`.
- [x] `src-tauri/src/ports/{mod,clipboard,store,frontmost}.rs` -- `ClipboardReader` (formats, text), `Store` (`record`, `list_recent`), `Frontmost` (app name); fakes under `cfg(test)`.
- [x] `src-tauri/src/adapters/sqlite/{mod,migrations}.rs` -- `SqliteStore` (WAL), schema v1, upsert by `content_hash` via `INSERT … ON CONFLICT DO UPDATE`.
- [x] `src-tauri/src/adapters/clipboard.rs` -- clipboard-rs reader + watcher (500 ms on macOS).
- [x] `src-tauri/src/adapters/macos.rs` / `adapters/windows.rs` -- frontmost app name (Windows: `None` until stage g).
- [x] `src-tauri/src/services/capture.rs` -- `capture_once(reader, frontmost, store, now)`: read text → skip blank → record; watcher loop with `catch_unwind` + backoff; emit `history-changed` via callback.
- [x] `src-tauri/src/shell/{mod,commands,state}.rs` -- `list_items(limit)` → `Vec<ItemSummary>`; `AppState` holds store or startup error; `startup_error` command.
- [x] `src/api.ts`, `src/App.tsx`, `src/components/HistoryList.tsx`, `src/App.css` -- typed API, list with relative time + source app, ↑/↓ selection by id, refetch on events, error banner.
- [x] tests -- dedupe key, store upsert/bump/order/preview, capture_once with fakes (blank, non-text, new, re-copy).

**Acceptance Criteria:**
- Given the app is running, when the user copies text in another app and opens the window, then the item is at the top with its source app (macOS) within one second.
- Given the window is open, when a new copy lands, then the list updates without losing the selected item.
- Given the window is shown again, then the selection resets to the first item.
- Given the app restarts, then history is still listed (persisted in `app_local_data_dir/magpie.db`).

## Implementation Notes

- Removed `panic = "abort"` from the release profile: with it, `catch_unwind` cannot contain a watcher panic and AD-18's restart would never run in release builds.
- `ports/clipboard.rs` also defines `ClipboardWatch` (blocking change source) so `services::capture` can run the watch loop and supervisor without naming the clipboard-rs adapter (AD-1).
- Schema v1 has no `settings` table yet; it arrives with stage f as migration v2.
- Stage b records plain text whenever present, so a multi-format copy (HTML + text, Finder files + names) is stored as `text` until stage e applies AD-13.
- Verified the real clipboard-rs/NSWorkspace adapters once with a throwaway ignored test (pbcopy): launch content not captured, blank skipped, re-copy bumped one row, source app detected. Not run through `tauri dev` because an installed Magpie.app instance was running and single-instance would hand off to it.

- Orchestrator end-to-end runs (macOS, `tauri dev`): text captured with source app, re-copy bumped to top as one row, whitespace-only skipped, list rendered newest first with source and relative time, selection on first item. Second run after review patches used an isolated `MAGPIE_DATA_DIR`.
- Added a debug-only `MAGPIE_DATA_DIR` override (ignored in release) and skip single-instance for such runs, so test runs never touch real history or hand off to an installed copy.
- Idle CPU on the debug dev build is ~2%; sampling shows the capture-watcher and main threads idle (timed wait / run loop), remainder in WebKit/JSC/CoreVideo threads of the dev webview. Release-build idle measurement is pending (stage c demo build). The stage a release build measured 0.0%.

## Spec Change Log

## Review Triage Log

Layers: blind-hunter, edge-case-hunter, verification-gap. No intent_gap or bad_spec; 12 patch entries applied by the implementer, 4 deferred to `_bmad-output/implementation-artifacts/deferred-work.md`.

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | Source app recorded as Magpie itself | medium | patch | Observed in a real run ("magpie" source). Fixed: own pid returns None. |
| 2 | No autorelease pool around NSWorkspace calls on watcher thread | medium | patch | Long-lived non-main thread; autoreleased objects never drained. Fixed. |
| 3 | Bump with unknown source erases known source_app | medium | patch | `SET source_app = excluded.source_app` writes NULL. Fixed with COALESCE + tests. |
| 4 | Sync commands block main thread on store lock | medium | patch | Sync Tauri commands run on main thread. Fixed: `command(async)`. |
| 5 | Out-of-order refresh responses overwrite newer list | medium | patch | Two concurrent `refresh` calls, no ordering guard. Fixed: request counter. |
| 6 | Arrow handling swallows modifier/IME keys | medium | patch | Shift+Up text selection broken in search box. Fixed. |
| 7 | Negative user_version skips migrations | low | patch | `current as usize` wraps. Fixed + test. |
| 8 | Empty preview after 2000+ leading whitespace | low | patch | `substr` before trim. Fixed with SQL ltrim + test. |
| 9 | Fake store tie order differs from SQLite | low | patch | Stable sort vs `seq DESC`. Fixed + tie test. |
| 10 | DB-unavailable path untested | medium | patch | No test reached `startup_error`/`list_items` Err path. Fixed: plain fns + 2 tests. |
| 11 | Doubled error prefix in startup banner | low | patch | `AppError::Startup(CoreError.to_string())`. Fixed. |
| 12 | Relative times stale while window open | low | patch | `now` only updated on refresh. Fixed: 30 s tick. |
| 13 | Reset selection not scrolled into view | low | patch | Effect keyed only on `selectedId`. Fixed. |
| 14 | Screen readers don't announce selection | low | patch | No option ids / activedescendant. Fixed (combobox pattern). |
| 15 | No size cap on captured text | medium | defer | Real; cap is a settings decision for stage f. |
| 16 | Frontend selection logic untested | medium | defer | No JS test runner in repo. |
| 17 | Release `panic` setting unverified by tests | medium | defer | Needs release smoke test. |
| 18 | Copies merged within a poll / lost during restart backoff | low | defer | Inherent to polling; documented in DECISIONS.md. |
| 19 | Dedupe key not canonicalised (trailing newline, CRLF) | low | reject | Raw bytes are the chosen canonical form for text (recorded in DECISIONS.md); different bytes are different copies. |
| 20 | Clock moving backwards prevents bump to top | low | reject | Rare, and MAX() would equally fail to move it to the top. |
| 21 | WAL may silently not apply | low | reject | Local app data dir on local disk; no network FS in scope. |
| 22 | Watcher spawn failure is silent | low | reject | Thread spawn failure means OS resource exhaustion; logged; adding UI state is more than a direct fix. |
| 23 | Listeners may register after first event | false | reject | First `window-shown` needs a user hotkey press, long after mount. |
| 24 | `toISOString` RangeError on i64::MAX timestamp | false | reject | `now_ms` overflow fallback unreachable before year 292 million. |
| 25 | Removed window focus listener | false | reject | Window hides on blur, so focus only returns via `show()`, which emits `window-shown`. |
| 26 | Acceptance criteria never run in real app | false | reject | Orchestrator ran them end to end (see Implementation Notes). |

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- all pass
- `cd src-tauri && cargo clippy --all-targets -- -D warnings` -- clean
- `npm run build` -- tsc + vite succeed

**Manual checks (if no CLI):**
- Run `npm run tauri dev`, copy text in another app, open window via second launch: item listed with source app. Idle CPU of the process ~0%.
