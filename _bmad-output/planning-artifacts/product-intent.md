# Magpie: product intent (planning input)

This is the owner's brief, kept verbatim as the input to BMad planning.
Where any BMad planning artifact disagrees with this document, this
document wins.

The brief offered some choices in square brackets. The values that apply
are:

| Choice | Value |
| --- | --- |
| Angle | cross-platform (Windows + macOS first, Linux later), local-first, no account required |
| AI tool for BMad | claude-code |
| Frontend | React |
| Default hotkey | Cmd/Ctrl+Shift+V |
| Planning time box | a couple of hours |
| Licence | none until the owner chooses one |
| Develop on / verify on | macOS / Windows |

---

## What this is
I'm building a desktop clipboard manager in the same category as Paste
(https://pasteapp.io): it records everything the user copies, keeps it
searchable, and lets them paste any past item into the app they're
working in. Paste is Apple-only and subscription-based. Our angle is
cross-platform (Windows + macOS first, Linux later), local-first, no
account required. Use Paste as a reference for the category only: do
not copy its name, visual design, icons or copy.

The app is called Magpie. Keep the name in one config constant so it
can be changed later without a hunt.

## Stack (decided, don't revisit)
- Tauri 2 with a Rust backend
- Frontend: React + TypeScript + Vite
- SQLite for storage (rusqlite, with FTS5 for search)
- All clipboard capture, storage and search logic lives in Rust. The
  frontend is a thin view layer that calls Tauri commands.

Check the current stable versions and plugin APIs in the official docs
before you scaffold; don't rely on memory for Tauri plugin names or
signatures. Tauri's official clipboard plugin reads and writes but, as
far as I know, doesn't notify on change, so you will need to pick a
monitoring approach (a maintained community plugin, or native
listeners per OS). Evaluate, choose one, and write down why.

## MVP scope
1. Capture: a background listener records every clipboard change. Types:
   plain text, rich text/HTML, images, file paths. Store source app
   name and timestamp where the OS exposes them.
2. Dedupe: copying something already in history moves it to the top
   instead of creating a second entry.
3. History UI: a global hotkey (default Cmd/Ctrl+Shift+V, user
   configurable) opens a window listing items newest first, with type-
   appropriate previews. Fully keyboard-driven: arrows to move, Enter to
   paste, Esc to dismiss, typing starts a search.
4. Search: full-text over text items, results as you type. Target under
   50 ms on a history of 10,000 items.
5. Paste: selecting an item puts it on the clipboard, hides the window,
   returns focus to the previously active app, and pastes there. This
   needs synthetic keystrokes and, on macOS, the Accessibility
   permission. Build a clear first-run flow for granting it, and fall
   back to "copied, press paste yourself" if it is refused.
6. Pins: the user can pin items so they survive history cleanup, and
   view pinned items separately.
7. Privacy:
   - Skip items that password managers mark as concealed
     (org.nspasteboard.ConcealedType on macOS,
     ExcludeClipboardContentFromMonitorProcessing on Windows).
   - A user-editable list of apps to ignore.
   - A pause-capture toggle.
   - Data never leaves the machine. No telemetry, no network calls.
8. Retention: configurable limit by age and by count, with pinned items
   exempt. Store images on disk, not as blobs in the database, and
   delete the files when their rows expire.
9. Tray/menu bar icon with open, pause, settings and quit. Launch at
   login as an option.

## Out of scope for now
Cross-device sync, mobile apps, accounts, team sharing, AI features,
payments, auto-update. Don't build them, but don't make choices that
block them: keep the storage layer behind a trait, and give every item
a stable UUID.

## Quality bar
- Idle CPU near zero. Prefer OS change notifications over polling; if
  you must poll on a platform, say so and keep the interval sane.
- The hotkey window should appear in under 150 ms. Keep the window
  alive and hidden instead of recreating it.
- Rust: no unwrap() on anything touching the OS or the disk; errors
  surface to the UI as readable messages.
- Unit tests for dedupe, retention, search and the privacy filters.
  These must run without a real clipboard, so put the OS clipboard
  behind a trait and test against a fake.

## How to work
Build in this order, and get each stage working end to end before the
next:
  a. Scaffold, tray icon, hotkey opens an empty window
  b. Capture text into SQLite and list it
  c. Paste back into the previous app (checkpoint with the owner)
  d. Search, dedupe, pins
  e. Images, HTML, files
  f. Privacy filters, retention, settings screen
  g. Second platform pass and polish

Develop on macOS first, then verify on Windows. Anything that could not
be tested on a real machine for a given OS is listed as untested
instead of assumed to work.

Stop and ask the owner before: adding a dependency that is unmaintained
or carries a copyleft licence, changing the stack, or cutting anything
from the MVP scope. Everything else, decide and note it.

## What to hand back
- Everything pushed to the repo, with a README covering setup, build
  and the permission steps per OS
- DECISIONS.md: each non-obvious choice, the alternatives, and why
- The BMad planning artifacts in _bmad-output/, and a note on anywhere
  BMad's plan and this prompt disagreed
- A list of what is verified working per OS, what is untested, and
  known bugs

## Already done (stage a, before planning)
Stage (a) is built and on main: Tauri 2 scaffold, tray icon with Open
and Quit (Pause and Settings present but disabled), a global
CmdOrCtrl+Shift+V hotkey toggling a hidden, always-alive window,
Accessory activation policy on macOS (no Dock icon), single-instance,
strict CSP, product name read from `productName` in tauri.conf.json.

Clipboard monitoring was evaluated before planning: Magpie uses its own
watcher on `clipboard-rs` 0.3.5 (MIT). macOS has no clipboard-change
notification, so it polls `NSPasteboard.changeCount` at 500 ms; Windows
uses the native `WM_CLIPBOARDUPDATE` listener.
