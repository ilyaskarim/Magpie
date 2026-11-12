---
id: SPEC-magpie-mvp
companions:
  - stack.md
  - build-stages.md
  - ../../planning-artifacts/product-intent.md
  - ../../planning-artifacts/architecture/architecture-Magpie-2026-11-13/ARCHITECTURE-SPINE.md
sources: []
---

> **Canonical contract.** This SPEC and the files in `companions:` are the complete, preservation-validated contract for what to build, test, and validate. The owner brief (`product-intent.md`) is kept as a companion because it wins any disagreement with this spec.

# Magpie MVP

## Why

A vision plus an opportunity. Paste-style clipboard history is valuable, but the leading product is Apple-only and needs a subscription. Magpie gives the same everyday workflow (copy, find later, paste anywhere) on Windows and macOS, with all data kept on the user's machine and no account.

## Capabilities

- **CAP-1**
  - **intent:** The app records every clipboard change in the background: plain text, rich text/HTML, images and file paths, with timestamp and source app where the OS exposes it.
  - **success:** Copying each of the four types in another app produces one new history entry of the right type, with a timestamp and (where available) the source app name.
- **CAP-2**
  - **intent:** Copying content that is already in history moves the existing item to the top instead of adding a duplicate.
  - **success:** Copying the same content twice leaves one entry, now newest, with the same UUID.
- **CAP-3**
  - **intent:** A global hotkey (default CmdOrCtrl+Shift+V, user-configurable) opens a fully keyboard-driven history window listing items newest first with type-appropriate previews.
  - **success:** Hotkey shows the window in under 150 ms; arrows move the selection, Enter pastes, Esc dismisses, typing goes to search; text, HTML, image and file items each render a recognisable preview.
- **CAP-4**
  - **intent:** The user finds past text items by full-text search, with results updating as they type.
  - **success:** A query against 10,000 items returns in under 50 ms (measured in the Rust layer).
- **CAP-5**
  - **intent:** Choosing an item puts it on the clipboard, hides the window, returns focus to the previously active app and pastes there.
  - **success:** With permission granted, Enter on an item inserts it into the previously focused text field. On macOS a first-run flow explains and requests Accessibility; if refused, the item is still copied and the UI says "copied, press paste yourself".
- **CAP-6**
  - **intent:** The user can pin and unpin items, pinned items survive cleanup, and pinned items can be viewed on their own.
  - **success:** A pinned item survives a retention run that removes all its unpinned peers, and appears in the pinned view.
- **CAP-7**
  - **intent:** Sensitive copies are not recorded: concealed items from password managers, items from user-listed ignored apps, and anything copied while capture is paused.
  - **success:** Fake-clipboard tests show items with `org.nspasteboard.ConcealedType` / `ExcludeClipboardContentFromMonitorProcessing`, from an ignored app, or during pause are not stored.
- **CAP-8**
  - **intent:** History is trimmed by configurable maximum age and maximum count; pinned items are exempt; image data lives as files on disk and is deleted with its row.
  - **success:** After a retention run, no unpinned item is older than the age limit, the unpinned count is within the count limit, and no orphaned image files remain.
- **CAP-9**
  - **intent:** A tray/menu bar icon offers open, pause/resume capture, settings and quit; the app can optionally launch at login.
  - **success:** Each tray item works; enabling launch at login makes the app start after logging in again.
- **CAP-10**
  - **intent:** A settings screen lets the user change the hotkey, ignored apps, retention limits, launch at login and pause state.
  - **success:** Each setting changed in the UI takes effect without a restart and persists across restarts; invalid input (e.g. an unusable hotkey) shows a readable error.

## Constraints

- Stack is fixed: Tauri 2 + Rust, React + TypeScript + Vite, SQLite via rusqlite with FTS5. See `stack.md`.
- All capture, storage and search logic is in Rust; the frontend only renders and calls Tauri commands.
- No network calls, no telemetry, no account. Data never leaves the machine.
- Storage sits behind a trait and every item has a stable UUID, so sync can be added later.
- Idle CPU near zero: OS change notifications where they exist; polling only where they do not, at a documented, sane interval.
- The history window is created once and kept hidden; it is never recreated.
- No `unwrap()` on anything touching the OS or disk; errors reach the UI as readable messages.
- The OS clipboard sits behind a trait; dedupe, retention, search and privacy tests run against a fake.
- The product name is defined in one place; nothing is copied from Paste's name, design, icons or copy.
- macOS and Windows for the MVP. Anything not run on a real machine for an OS is reported as untested.
- Adding an unmaintained or copyleft dependency, changing the stack, or cutting MVP scope requires the owner's approval first.

## Non-goals

- Cross-device sync, mobile apps, accounts, team sharing.
- AI features, payments, auto-update.
- Linux support (later; do not block it).

## Success signal

On a Mac and a Windows PC, a user copies text, a web snippet, a screenshot and a file, presses the hotkey, types a few letters of an earlier item, presses Enter, and that item is pasted into the app they were in, with the CPU near idle between copies and no network traffic from the app.

## Assumptions

- Settings persist locally in the app data directory.
- Rich text is stored as HTML plus its plain-text form; RTF-only copies keep the plain-text form.
- Default retention is 90 days and 5,000 items, both editable (the brief gives no defaults).
