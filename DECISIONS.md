# Decisions

Non-obvious choices, the alternatives considered, and why. Newest
decisions are added at the end of each section. Architecture decisions
with `AD-n` IDs are defined in
`_bmad-output/planning-artifacts/architecture/architecture-Magpie-2026-11-13/ARCHITECTURE-SPINE.md`.

## Tooling and process

### BMad Method v6.12.4 (stable), not v7

- **Chosen:** `npx bmad-method@6.12.4 install --directory . --modules bmm --tools claude-code --yes`.
- **Alternatives:** v7, which the install docs now lead with
  (`npx skills add …` then `bmad setup`).
- **Why:** the docs label v7 a "preview alpha" whose install path changes
  often, and offer v6.12 as the stable option. The stable CLI supports the
  exact non-interactive flags in the brief. Migrating to v7 later is a
  documented path (`bmad migrate`).

### Personal BMad config not committed

- **Chosen:** `_bmad/config.user.toml`, `_bmad/core/config.yaml` and
  `_bmad/bmm/config.yaml` are gitignored.
- **Why:** they contain the installing user's name. Skills resolve config
  from `_bmad/config.toml` plus optional user layers, and fall back to
  shared defaults when the personal file is missing (checked with
  `resolve_config.py`). The README tells new clones how to regenerate it.

### Lean BMad planning path

- **Chosen:** `bmad-spec` (SPEC.md + stories.yaml) then
  `bmad-architecture` (spine), one pass each.
- **Alternatives:** the full path `bmad-help` recommends: `bmad-prd` →
  `bmad-architecture` → `bmad-create-epics-and-stories` →
  `bmad-sprint-planning`.
- **Why:** the brief already fixes stack, scope, out-of-scope and build
  order and asks for proportionate planning. A PRD would restate the
  brief, and the stages a–g already are the epics. `bmad-help` itself
  names `bmad-spec` as the leaner alternative.

## App shell

### Product name in one place

- **Chosen:** `productName` in `src-tauri/tauri.conf.json`. Rust reads it
  via `app_name()`, the frontend via `getName()`.
- **Alternatives:** a Rust `const` plus a TS constant (two places), or a
  generated file.
- **Why:** Tauri already needs `productName` for the bundle and window
  title, so any other constant would be a second copy to keep in sync.

### Menu-bar-only app on macOS; single instance

- **Chosen:** `ActivationPolicy::Accessory` (no Dock icon), plus
  `tauri-plugin-single-instance` so a second launch opens the existing
  window.
- **Why:** a clipboard manager lives in the background. Two instances
  would both record the clipboard and fight over the hotkey.

### One window, created at startup, hidden and shown

- **Chosen:** the `main` window is declared in `tauri.conf.json` with
  `visible: false`; close and focus loss hide it.
- **Why:** the brief's under-150 ms target. Creating a webview on demand
  costs far more than showing a hidden one.

### Tray click behaviour per OS

- **Chosen:** macOS: any click opens the menu. Windows: left click opens
  the history window, right click opens the menu.
- **Why:** each matches its platform's convention.

### No opener plugin; strict CSP

- **Chosen:** removed the scaffold's `tauri-plugin-opener`; the CSP allows
  only `'self'`, IPC and local asset/data images.
- **Why:** the app must make no network calls. The CSP makes an accidental
  remote request from the webview fail.

## Clipboard

### Own watcher on `clipboard-rs`, not a community Tauri plugin

- **Chosen:** `clipboard-rs` 0.3.5 (MIT) for reading, writing and change
  watching, wrapped in our own `Clipboard` port.
- **Alternatives:**
  - `tauri-plugin-clipboard` (CrossCopy): no commits since 2024-10.
  - `tauri-plugin-clipboard-x`: one maintainer, pins `clipboard-rs` 0.2.
  - `clipboard-master` + `arboard`: signal only, plus a second crate to
    read.
  - The official `tauri-plugin-clipboard-manager`: no change notification.
- **Why:** both community plugins are thin wrappers over `clipboard-rs`,
  but they hide the list of available formats, and the privacy filter
  needs that list to see concealed markers before reading content. Using
  the crate directly gives us that list, raw custom-format buffers, and
  one dependency instead of two.

### macOS polls; Windows does not

- **Chosen:** macOS checks `NSPasteboard.changeCount` every 500 ms and
  reads contents only when it changes. Windows uses
  `AddClipboardFormatListener` / `WM_CLIPBOARDUPDATE` with no polling.
- **Why:** macOS has no clipboard-change notification API. Reading an
  integer every 500 ms is negligible CPU, and it is the interval
  established tools such as Maccy use.

### Synthetic paste without `enigo`

- **Chosen:** call `CGEventPost` (macOS) and `SendInput` (Windows)
  directly for the single Cmd/Ctrl+V keystroke.
- **Alternatives:** `enigo` (MIT, maintained).
- **Why:** we need one key chord. `enigo` does not export its Accessibility
  check, which we need for the first-run flow, and the direct calls are
  about 30 lines per OS.

## Architecture (from the BMad review gate)

The full set is AD-1 to AD-18 in the architecture spine. These are the
ones where a reasonable person might have chosen differently:

### Pause survives a restart

- **Chosen:** `paused` is a persisted setting.
- **Alternative:** reset to capturing on every launch.
- **Why:** people pause to protect something sensitive. Silently
  resuming after a reboot or crash would break that expectation. The tray
  shows the paused state, so it is easy to notice.

### Clipboard content present at launch is not captured

- **Why:** Magpie cannot know where it came from (no source app, possibly
  copied before the user's ignore rules applied), and it may be something
  the user copied before pausing.

### Own writes identified by clipboard sequence number

- **Chosen:** after writing to the clipboard for paste-back, record the
  OS sequence number (`changeCount` / `GetClipboardSequenceNumber`) and
  ignore changes at or below it.
- **Alternatives:** compare content hashes, or "skip the next change".
- **Why:** the OS can rewrite what we wrote (CF_HTML headers, image
  re-encoding), so a hash match fails. A skip flag can swallow a real
  copy that lands at the same moment.

### Storage in the local, non-roaming app data directory

- **Why:** on Windows the roaming profile is synced on domain networks.
  A clipboard history database must stay on the machine.

### macOS bindings from the `objc2` family

- **Chosen:** `objc2-app-kit`, `objc2-core-graphics`,
  `objc2-application-services`, rather than the older `core-graphics`
  crate.
- **Why:** Tauri already depends on the `objc2` crates, so this adds no
  second set of CoreFoundation types and keeps one binding family.

### Release builds unwind on panic

- **Chosen:** no `panic = "abort"` in `[profile.release]`.
- **Alternative:** keep the scaffold's `panic = "abort"` for a slightly
  smaller binary.
- **Why:** AD-18 restarts the capture watcher after a panic via
  `catch_unwind`, which does nothing when panics abort the process.

### Text dedupe uses the raw bytes

- **Chosen:** the dedupe key for text is the exact UTF-8 bytes; `hello`
  and `hello\n`, or CRLF and LF versions, are different items.
- **Alternative:** normalise line endings and trailing whitespace first.
- **Why:** paste-back must return exactly what was copied. Merging
  near-identical copies would silently change what gets pasted.

### Polling merges very fast copies (macOS)

- Copies made within one 500 ms poll keep only the last one, and copies
  made while the watcher is restarting after a crash are not captured.
  This is the cost of polling on macOS; Windows uses change notifications.

### Isolated debug runs

- **Chosen:** debug builds honour `MAGPIE_DATA_DIR` (history stored there)
  and then skip the single-instance check. Release builds ignore it.
- **Why:** test runs must never write to a user's real history, and must
  be able to run next to an installed copy.

### Accessibility explainer answered: a marker file

- **Chosen:** `<root>/accessibility-asked`, created after the system
  prompt was shown, after the settings pane actually opened, or on "Not
  now". Once it exists, "Open Accessibility settings" opens the pane
  directly instead of the one-time system prompt.
- **Alternatives:** a `settings` table now; an in-memory flag.
- **Why:** the settings table and `SettingsService` arrive in stage f, and
  an in-memory flag would show the explainer on every launch. The marker
  can move into `Settings` when that exists.

### Paste-back without the permission: copy, say so, then return focus

- **Chosen:** when Accessibility is not granted (or no app was saved), Enter
  copies the item and moves it to the top but keeps the window open. The
  window shows "Copied — press ⌘V to paste" (Ctrl+V off macOS; "Copied —
  paste it where you need it" when there is no saved app) for about 1.5 s,
  then `return_to_previous_app` hides it and reactivates the saved app.
  Only the trusted path hides at once and goes on to activate and paste;
  if that later fails (app gone, timeout, keystroke failed), the notice
  appears the next time the window opens.
- **Alternatives:** hide at once and show the notice only on the next open;
  hide at once and reactivate the saved app without telling the user.
- **Why:** the user has to learn at paste time that they must press paste
  themselves, and returning focus afterwards means that is one keystroke.
  The keystroke is sent only when the saved app (same pid and bundle id)
  is confirmed frontmost, so it never lands in the wrong app.

### Search: word-prefix matching, newest first

- **Choice:** each word typed becomes a quoted FTS5 prefix term, and all
  must match (`hel wor` finds "hello world"). Case and accents are
  ignored. Results stay in recency order, not relevance order. User input
  is always quoted, so FTS5 operators (`AND`, `NEAR`, `*`, `col:`) are
  plain text, and the query is passed as a bound parameter.
- **Alternatives:** substring matching inside words (`LIKE '%x%'` or a
  trigram index); ranking by relevance (bm25).
- **Why:** prefix matching uses the existing FTS5 index and stays under
  50 ms at 10,000 items (13 ms measured). A clipboard history is mostly
  recalled by "the thing I copied recently", so recency order beats
  relevance order.
