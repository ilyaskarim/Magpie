---
title: 'Stage d: search, dedupe, pins'
type: 'feature'
created: '2026-11-13'
status: 'done'
baseline_commit: 'fbf96bc5328fc65bd251fe83533ad6329324b6f8'
route: 'dispatch'
review_loop_iteration: 1
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-Magpie-2026-11-13/ARCHITECTURE-SPINE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** With more than a screenful of history the user cannot find an older item, and nothing can be kept safe from future cleanup (CAP-4, CAP-6). Dedupe (CAP-2) landed in stage b but has no test that a pinned item stays pinned when re-copied.

**Approach:** Typing in the window's search box filters history as you type through one Rust `search` command backed by the existing FTS5 index (AD-8). Items can be pinned and unpinned from the keyboard, show a pin marker, and a Pinned view lists only pinned items; search works inside it too.

## Boundaries & Constraints

**Always:** AD-8 (FTS5 over plain text; each user token becomes a quoted prefix term, ANDed; empty query = recency list; one `search` command with `pinned_only`), AD-2 (UI refetches on `history-changed`; `set_pinned` emits it), AD-9 (snake_case commands, camelCase DTOs, `src/api.ts` only), AD-17 (selection by id; `window-shown` resets query, view and selection). Results are ordered by `last_copied_at` DESC, `seq` DESC (recency, not relevance). Search must take under 50 ms for a 10,000-item history, measured in Rust on an optimised build. User input must never be able to inject FTS5 syntax or SQL.

**Never:** retention or deleting items (stage f); non-text kinds (stage e); match highlighting or fuzzy/substring-inside-word matching (FTS prefix matching is the decided behaviour); a client-side filter.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Prefix search | items "hello world", "help me", "yellow"; query `hel` | "hello world", "help me" (newest first); not "yellow" | N/A |
| Multi-token AND | query `wor hel` | "hello world" only | N/A |
| Diacritics / case | item "Café Crème"; query `cafe` | match | N/A |
| FTS syntax in input | query `"`, `*`, `AND`, `NEAR(`, `col:x`, `-x`, `^` | treated as literal text; no error, no crash | N/A |
| Empty / whitespace query | `""` or `"   "` | recency list | N/A |
| Pinned view | 3 items, 1 pinned; pinned_only | only the pinned item; search inside it filters pinned only | N/A |
| Pin toggle | ⌘P (Ctrl+P off macOS) on selected item | pinned flag flips, marker shows, `history-changed` emitted | unknown id → readable error |
| Re-copy pinned | pinned "x" copied again | same id, still pinned, moved to top | N/A |
| Scale | 10,000 items, typical 1–3 token queries | each search < 50 ms (release build) | N/A |

</frozen-after-approval>

## Code Map

- `src-tauri/src/adapters/sqlite/{mod,migrations}.rs` -- `items_fts` (content=items, content_rowid=seq, unicode61 remove_diacritics 2) and triggers already exist; `list_recent` builds `ItemSummary` with preview trimming in SQL — reuse that row mapping for search. No schema change needed.
- `src-tauri/src/ports/store.rs` -- `Store` trait + `MemoryStore` fake; add `search` and `set_pinned`.
- `src-tauri/src/shell/commands.rs` -- plain-fn + `#[tauri::command(async)]` pattern; `list_items` exists (keep it or make it call search with an empty query).
- `src/App.tsx`, `src/api.ts`, `src/components/HistoryList.tsx` -- request-counter refresh, selection by id, notice/permission UI; the search input already exists but filters nothing.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/domain/search.rs` -- pure `fts_query(user_input) -> Option<String>`: split on whitespace, drop empty tokens, wrap each as `"tok"*` with internal `"` doubled, join with spaces (implicit AND); `None` for empty input. Unit tests incl. every syntax-injection row.
- [x] `src-tauri/src/ports/store.rs`, `adapters/sqlite/mod.rs` -- `search(query: Option<&str>, pinned_only: bool, limit) -> Vec<ItemSummary>` and `set_pinned(id, bool)`; `NotFound` for unknown id; fake mirrors behaviour (fake may use a simple case-insensitive prefix match).
- [x] `src-tauri/src/shell/commands.rs`, `lib.rs` -- `search(query, pinned_only, limit)` and `set_pinned(id, pinned)` commands; emit `history-changed` after `set_pinned`.
- [x] `src/api.ts`, `src/App.tsx`, `src/components/HistoryList.tsx`, `src/App.css` -- search as you type (stale responses ignored), All/Pinned switch (Tab toggles; also clickable), ⌘P/Ctrl+P toggles pin on the selected item, pin marker per row, empty-state text per view ("No matches", "No pinned items"); `window-shown` resets query and view to All.
- [x] tests -- matrix rows in Rust (SQLite store), plus a 10,000-item timing test that runs under `cargo test --release` and asserts each of several queries < 50 ms (skipped/relaxed in debug builds with a clear message).

**Acceptance Criteria:**
- Given 10,000 items, when the user types a query, then results update on each keystroke without visible lag.
- Given an item is pinned, when the user switches to Pinned, then it is listed there and Enter pastes it as usual.

## Implementation Notes

- `list_items` and `listItems` were removed after review; the UI uses `search` with an empty query. `Store::list_recent` remains as a test-only helper.
- Punctuation-only input (e.g. `"` or `*`) yields an FTS phrase with no tokens: no error, zero results.
- Tab toggles All/Pinned only when focus is not on a button, so the permission prompt keeps keyboard focus movement.
- `set_pinned` emits `history-changed` on failure too (a missing item means the UI list is stale), like `paste_item`.
- Timing (10,000 items, limit 500, Apple Silicon): slowest query 13 ms release, 38 ms debug (debug bound 250 ms).

## Spec Change Log

## Review Triage Log

Iteration 1 (blind-hunter, edge-case-hunter, verification-gap):

- patch — NUL in query breaks the FTS5 string → treat NUL as whitespace in `fts_query`.
- patch — history-changed refresh racing a keystroke/window-shown refresh drops the selection reset (also breaks AD-17 on reopen) → pending-reset ref.
- patch — failed current search leaves the old list selectable → clear items and selection.
- patch — repeated ⌘P sends a stale value → ignore key repeat, in-flight guard.
- patch — ⌘P matched on `e.key` fails on non-Latin layouts → `e.code === "KeyP"`.
- patch — Tab steals focus from permission/notice buttons → no toggle while they show.
- patch — fake store tokenises queries differently from unicode61 → split query tokens too; diacritics noted.
- patch — debug timing bound 1000 ms hides large regressions → 250 ms.
- patch — unused `listItems` / `list_items` → removed.
- patch — pin marker named twice; hint omits unpin → one accessible name, "pins/unpins".
- defer — no test that `set_pinned` emits `history-changed` (no Tauri mock harness; same gap as `paste_item`).
- defer — release-mode 50 ms check runs only by hand (no CI yet).
- defer — frontend behaviour untested (existing deferred item: frontend test runner).
- defer — full ARIA tab pattern for the All/Pinned switch (stage g polish).
- reject — FTS `prefix=` index: slowest query 13 ms at 10k; no schema change needed now.
- reject — platform detection via user agent: reliable inside the Tauri webview; revisit in stage g if needed.
- reject — Linux path in CLAUDE.md: Linux is not a target.
- handled outside the diff — version bump, `CLAUDE.md` in its own commit, story status bookkeeping.

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- all pass
- `cd src-tauri && cargo test --release search` -- timing test passes (< 50 ms per query at 10,000 items)
- `cd src-tauri && cargo clippy --all-targets -- -D warnings` -- clean
- `npm run build` -- succeeds

**Manual checks (if no CLI):**
- `MAGPIE_DATA_DIR=/tmp/magpie-dev npm run tauri dev`: type to filter, Tab to Pinned, ⌘P pins/unpins.
