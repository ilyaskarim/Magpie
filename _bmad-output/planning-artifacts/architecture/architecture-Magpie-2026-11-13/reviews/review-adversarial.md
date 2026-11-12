# Adversarial review: Architecture Spine, Magpie MVP

Reviewed: `ARCHITECTURE-SPINE.md` (2026-11-13) against `SPEC.md`, `build-stages.md` and the stage-(a) code in `src-tauri/src/`.

**Verdict:** The spine is sound and appropriately short. All 10 capabilities are mapped and most real divergence points are pinned. Before stages b–g, four holes need closing: pause ownership, how self-writes are detected, which kind a multi-format copy gets, and error layering against the stage-(a) code. A handful of smaller gaps are listed after those.

Each finding names two units that both obey every AD and still build incompatibly, or one AD that does not prevent the problem it is meant to prevent.

---

## High

### H1. Pause state has no single owner (CAP-7, CAP-9, CAP-10)
- **Pair:** Stage f tray work implements "Pause capture" as an `AtomicBool` in managed state, flipped directly by the tray menu handler. Tray events are Rust callbacks, not commands, so AD-2 does not obviously apply. Meanwhile the stage f settings screen stores `paused` inside `Settings` (AD-10 lists pause under CAP-10) and changes it through `update_settings`. AD-6 just takes a `paused` argument, so the capture loop can read either one. The result is that the tray and the settings screen disagree, the tray label goes stale, and only one of the two survives a restart.
- **Also open:** Should pause persist across restarts? CAP-10 says every setting persists. A pause that silently survives a reboot is a privacy and usability trap. That is a decision, not an implementation detail.
- **Fix (tighten AD-10):** "`Settings` (including `paused` and `ignored_apps`) has one in-memory owner, `SettingsService` (`Arc<RwLock<Settings>>`), loaded at startup and written through to the store. The tray, commands and hotkey change it only through `SettingsService::update` (the same path as `update_settings`), which emits `settings-changed`. The capture loop reads a snapshot from the service for each clipboard change and never reads the DB. Whether `paused` persists: [decide]."

### H2. AD-7's "expect that change" mechanism is undefined and fails as a content match (CAP-1, CAP-5)
- **Pair:** The stage c paste service sets an expected hash. The capture loop compares the hash of the next change against it. But the OS can rewrite what Magpie writes. Windows wraps HTML in a CF_HTML header. macOS can convert a PNG to TIFF or change its colour space, so the decoded RGBA differs. Browsers' HTML does not round-trip. The hash then misses, and the loop creates a new row with `source_app = Magpie`, which is exactly what AD-7 is meant to stop. A second team instead uses a "skip next change" boolean. That one swallows a real user copy if the write fails, and on Windows it misfires because several `WM_CLIPBOARDUPDATE` messages can arrive for one multi-format write.
- **Contradiction:** AD-7 says a self-write keeps `source_app`. AD-3 says a dedupe hit updates `source_app`. A unit following AD-3 overwrites it.
- **Fix (tighten AD-7):** "The paste service bumps the item itself (`Store::touch(id)`: `last_copied_at` only), then writes the clipboard and records the resulting OS change token (macOS `changeCount`, Windows `GetClipboardSequenceNumber`) in a shared `SelfWrite` slot. The capture loop drops any change whose token matches or is older than that slot, without reading any content. AD-3's `source_app` update does not apply to self-writes."

### H3. No rule for which kind a multi-format copy becomes (CAP-1, CAP-2, CAP-3, CAP-5)
- **Pair:** The stage e capture adapter classifies a browser image copy (image + HTML + text) as `image`. The stage e preview and paste unit writes back only the PNG. A different reading classifies it as `html`. A Finder copy (file URL + filename text + icon image) becomes `files` in one unit and `text` or `image` in another. Dedupe keys differ by kind (AD-3), so the same copy produces different rows depending on which unit wrote the code. Paste-back fidelity also differs: does an `html` item write HTML plus plain text, or only one of them?
- **Fix (new AD or extend AD-3):** "Kind is chosen by fixed precedence: `files` > `image` > `html` > `text`. An item stores every representation it uses: an `html` item also stores `plain_text`, and a `files` item stores `plain_text` = newline-joined paths for FTS. RTF-only copies become `text`. Paste-back writes every stored representation (HTML + plain; PNG; file list + plain)."

### H4. `AppError` placement contradicts AD-1 and the stage-(a) code
- **Pair:** The stage-(a) `AppError` has a `Tauri(#[from] tauri::Error)` variant and lives at the crate root (the seed puts it in `shell/error`). The Consistency table says errors are "`AppError`". A stage b store adapter that obeys the table returns `AppError`. That pulls Tauri into `ports/` and `domain/` (breaking AD-1) and makes `domain` depend on `shell` (breaking the dependency arrows). A second unit defines its own `StoreError`. The result is two error types and no rule for converting between them.
- **Fix (tighten the Errors convention):** "`domain`/`ports` define `CoreError` (thiserror, no Tauri, OS or rusqlite types). Adapters map their errors into it. `shell::AppError` wraps `CoreError` via `From` and is the only type returned by commands. Stage b moves `error.rs`, `hotkey.rs`, `tray.rs` and `window.rs` into `shell/` as its first step."

---

## Medium

### M1. FTS keyed by implicit rowid on a TEXT-PK table breaks after VACUUM
The ERD gives `items` a `text id PK`, and `ITEMS_FTS` joins on `rowid` via triggers. SQLite's `VACUUM` may renumber the implicit rowid of tables without an `INTEGER PRIMARY KEY`, which silently desynchronises an external-content FTS5 index. Upserts written as `INSERT OR REPLACE` would also delete and re-insert the row, resetting `pinned` and churning the FTS index.
**Fix (AD-8):** Add `seq INTEGER PRIMARY KEY` to `items`, with `id TEXT UNIQUE`, and set FTS `content_rowid='seq'`. Dedupe hits are `UPDATE`s; `INSERT OR REPLACE` is forbidden. Escape `"` inside each quoted token.

### M2. Retention: which timestamp counts, and when it runs, are unspecified (CAP-8)
One unit ages items by `created_at`. Another ages them by `last_copied_at`, so a re-copied old item is kept by one and deleted by the other. Retention timing is also unspecified. It could run inline in the capture loop while holding the lock, on a timer, or only at startup. If it runs only at startup, CAP-10's "takes effect without a restart" fails when the limits are lowered.
**Fix (new rule under AD-10, or a new AD):** "Age is measured on `last_copied_at`. `services::retention::run` executes at startup (after the orphan sweep), after every successful insert (count check), after any `update_settings` that changes limits, and hourly. It deletes in one transaction and emits `history-changed`."

### M3. The event contract and window/selection state are not fixed (CAP-3, CAP-9)
AD-2 says "refetch on `history-changed`", but the following are left open:
- (a) Should a refetch while the user is arrowing re-run the current `search(query)`, or call `list_items`? Two commands give the same result for an empty query, so units will drift.
- (b) Is the selection kept by index (it jumps when a new capture lands on top) or by `id`?
- (c) How does the UI learn that the window was just shown, so it can reset query and selection?
- (d) How does the tray "Settings…" item navigate the single, never-recreated window to the settings view?

**Fix (AD-9):** List the events and their payloads: `history-changed` (none), `settings-changed` (`Settings`), `window-shown` (`{ view: "history" | "settings" }`). Drop `list_items` in favour of `search("", pinnedOnly, limit)`. Rule: on `window-shown` the UI clears the query and selects the first item. On `history-changed` it re-runs the current query and keeps the selected `id` if it is still present, otherwise it selects the first item.

### M4. Stage-(a) `hotkey::register` cannot satisfy AD-10's "keep previous settings"
`register` calls `unregister_all()` before registering the new shortcut. If the new shortcut fails, the old one is already gone. A unit that obeys AD-10 by not saving the settings still leaves the user with no hotkey.
**Fix (AD-10):** "Side effects are applied transactionally. On failure, re-register the previous hotkey and autostart state before returning the error."

### M5. The image DTO and the app-data location are unspecified (CAP-3, CAP-8)
The row stores a relative path, but the UI needs a URL. One unit sends base64 (slow when listing 5,000 items). Another sends an absolute path and uses `convertFileSrc`, which requires an asset-protocol scope that `tauri.conf.json` does not yet have. `<app_data>` is also undefined. On Windows, `app_data_dir` is Roaming and `app_local_data_dir` is Local, and a clipboard database should not roam.
**Fix:** "Root = `app_local_data_dir()`; DB = `<root>/magpie.db`; images under `<root>/images/`. The item DTO carries `imageUrl` built by the shell with `convertFileSrc` semantics. The asset scope is limited to `<root>/images/**`. List DTOs carry a ≤200-char text preview, not the full payload."

---

## Low

- **L1 (AD-11):** "Held for one store operation" still allows PNG encoding and the file write to happen inside `Store::insert`, under the lock, which can block `search` past its 50 ms target on a large screenshot. Add: "Encoding and hashing happen before taking the lock."
- **L2 (AD-12):** The frontmost app is recorded only on the hotkey path. In the stage-(a) code, `window::show` is also reached from the tray and from single-instance. Record the frontmost app inside `window::show` (skipping Magpie itself) so that every path that shows the window records it.
- **L3 (CAP-5):** The command list has nothing for the Accessibility first-run flow (`get_paste_permission`, `request_paste_permission`) or for pause. Units will invent their own names. Add them to AD-9.
- **L4 (startup and shutdown):** The startup order is unstated. Suggested: open DB → migrate → orphan sweep → retention → load settings → register hotkey from settings (stage-(a) uses `DEFAULT_SHORTCUT`) → start watcher. Also state whether the clipboard contents present at launch are captured.

## Checked and fine
- Deferred items cannot cause divergence.
- The capability map covers CAP-1 to CAP-10.
- Time (AD-4), stable IDs (AD-3) and schema migrations (`user_version`) are adequately pinned.
- Single-writer (AD-2) plus Tauri managed state is enforceable.
