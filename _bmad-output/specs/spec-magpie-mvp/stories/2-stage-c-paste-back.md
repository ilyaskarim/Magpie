---
title: 'Stage c: paste back into the previous app'
type: 'feature'
created: '2026-11-13'
status: 'in-review'
baseline_commit: 'acfd839ed220c6d20619ebd5194dac6218bc123a'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture/architecture-Magpie-2026-11-13/ARCHITECTURE-SPINE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Items can be listed but not used: there is no way to put a past item back into the app the user was working in (CAP-5).

**Approach:** Enter on the selected item (or a click) copies it to the clipboard, hides the window, reactivates the app that was frontmost when the window opened, and sends a synthetic Cmd+V. On macOS, a first-run flow explains and requests the Accessibility permission; without it the item is still copied and the UI says so.

## Boundaries & Constraints

**Always:** AD-7 (paste service bumps the item itself, records the OS clipboard sequence number after its write; capture loop drops changes at or below it), AD-12 (frontmost app recorded when the window is shown, before focus moves; cooperative activation on macOS 14+; wait until the saved app is frontmost, ~300 ms timeout, then paste), AD-13 (write every stored representation; for text-only items that is the plain text), AD-1/AD-14 (new `Paster` and clipboard-writer ports with fakes; `CoreError` below the shell), AD-2/AD-9 (commands + `history-changed`). macOS native code uses the objc2 family (`objc2-app-kit`, `objc2-core-graphics`, `objc2-application-services` 0.3). No `unwrap`/`expect` on OS paths.

**Never:** search, pins (stage d); non-text kinds (stage e); settings screen (stage f); Windows synthetic paste (stage g — Windows returns `CopiedOnly` for now); network calls (opening System Settings uses a local `x-apple.systempreferences:` URL via the OS, nothing else).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Paste granted | Accessibility trusted; previous app TextEdit; Enter on item | clipboard = item text; window hidden; TextEdit frontmost; text inserted; item moves to top; source app unchanged; no new row | N/A |
| Permission missing | not trusted; Enter | clipboard = item text; window stays open ~1.5 s showing "Copied — press ⌘V to paste", then hides and returns focus to the saved app; outcome `CopiedOnly` | N/A |
| First run | not trusted, never asked | window shows an explainer with "Open Accessibility settings" and "Not now"; choosing open triggers the system prompt / opens the pane | readable error if the pane cannot be opened |
| Previous app gone | saved app quit before Enter | copy done, no paste sent, outcome `CopiedOnly` | N/A |
| Activation timeout | saved app not frontmost within ~300 ms | no keystroke sent (never paste into the wrong app), outcome `CopiedOnly` | N/A |
| Own write | Magpie writes the clipboard | capture loop does not insert a row or change source_app | N/A |
| Item vanished | id not in store | readable error, window stays open | `CoreError::NotFound` → `AppError` |

</frozen-after-approval>

## Code Map

- `src-tauri/src/shell/window.rs::show` -- record frontmost app (pid + name) here before `show()/set_focus()`, but only when the frontmost app is not Magpie itself; keep it in managed state.
- `src-tauri/src/adapters/macos.rs` -- existing `SystemFrontmost` (autoreleasepool, own-pid filter); extend with pid lookup, Accessibility check (`AXIsProcessTrustedWithOptions` with prompt option), activation, `CGEvent` key posting (virtual key 9 = V, Command flag) to the HID event tap.
- `src-tauri/src/adapters/clipboard.rs` -- add write + sequence number (`NSPasteboard.generalPasteboard().changeCount()` on macOS, `GetClipboardSequenceNumber` on Windows via the `windows` 0.62 crate).
- `src-tauri/src/services/capture.rs::capture_once` -- consult a shared "own write" sequence (e.g. `Arc<AtomicI64>`) and skip changes at or below it.
- `src-tauri/src/adapters/sqlite/mod.rs` -- add `get_text(id)` and `touch(id, at)` (bump `last_copied_at` only) to the `Store` port and both stores.
- `src-tauri/src/shell/commands.rs` -- existing plain-fn + async-command pattern; follow it.
- `src/App.tsx`, `src/api.ts` -- Enter / click handling, notice banner, permission explainer.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/Cargo.toml` -- add macOS `objc2-core-graphics`, `objc2-application-services` (0.3); Windows `windows` 0.62 with `Win32_System_DataExchange` only.
- [x] `src-tauri/src/ports/{clipboard,paster,store}.rs` -- `ClipboardWriter` (`write_text`, `sequence`), `Paster` (`is_trusted`, `request_trust`, `activate(pid) -> bool`, `send_paste`), `Store::{get_text, touch}`; fakes.
- [x] `src-tauri/src/services/paste.rs` -- `paste(item_id, target, store, clipboard, paster, own_seq, now) -> PasteOutcome { Pasted, CopiedOnly(reason) }` implementing the AD-7/AD-12 order.
- [x] `src-tauri/src/services/capture.rs` -- own-write skip.
- [x] `src-tauri/src/adapters/{macos,windows,clipboard}.rs` -- implementations (Windows `Paster`: not trusted, `CopiedOnly`).
- [x] `src-tauri/src/shell/{window,commands,state,mod}.rs` -- saved target app; commands `paste_item(id)`, `accessibility_status() -> {trusted, asked}`, `request_accessibility()`, `open_accessibility_settings()`; emit `history-changed` after the bump.
- [x] `src/api.ts`, `src/App.tsx`, `src/components/PermissionPrompt.tsx`, `src/App.css` -- Enter and click paste; "Copied — press ⌘V to paste" notice; first-run explainer; readable errors.
- [x] tests -- paste service with fakes for every matrix row; capture own-write skip; store `get_text`/`touch`.

**Acceptance Criteria:**
- Given Accessibility is granted and TextEdit was frontmost, when the user opens Magpie with the hotkey and presses Enter on an item, then the text appears in TextEdit within ~0.5 s and Magpie's window is hidden.
- Given the permission was refused, when the user presses Enter, then the item is on the clipboard and the user is told to press ⌘V.
- Given a paste happened, then history has no new row for it and the pasted item is at the top.

## Implementation Notes

- **Ports.** `ClipboardWriter` is `write_text` plus a `ClipboardSequence` supertrait (`sequence() -> Option<i64>`), so the capture loop can read the sequence through the reader without also needing write access. `None` (Linux, or Windows without clipboard access) never counts as an own write. `Paster` gained a fifth method, `open_trust_settings`, so opening the pane stays inside the macOS adapter. `Frontmost` gained `app() -> Option<TargetApp>` (pid + name; `None` when Magpie itself is frontmost).
- **Paste order** (`services/paste.rs`, after review): `get_text` → `OwnWrites::begin` → write clipboard → record sequence (or `cancel` on error) → `touch` (only after a successful write). If not trusted or no saved target: return `CopiedOnly(notTrusted|noTarget)` with the window still open; the UI shows the inline notice for ~1.5 s, then `return_to_previous_app` hides and reactivates the saved app. Trusted path: `hide` → `activate(&TargetApp)` (bundle id must match the pid; ≤300 ms frontmost wait + 60 ms settle) → Cmd+V. Failures after hiding (`targetUnavailable`, `pasteFailed`) show on the next window show.
- **Own writes** (`OwnWrites`): a pending flag set before the write makes every change "own" until the sequence is recorded; afterwards a change is own only if it is at most 63 below the recorded sequence (survives counter resets). Residual: an external copy landing in the microseconds between Magpie's write and its sequence read is dropped.
- **Target lifetime:** the saved app is recorded on show (`window::remember`, never Magpie itself) and cleared on every hide.
- **macOS activation** runs `yieldActivationToApplication` (only if the selector exists, i.e. macOS 14+) and `activateWithOptions` on the main thread via `dispatch2::run_on_main`; the frontmost wait runs on the command's worker thread. `dispatch2` 0.3 (objc2 family) and `objc2-core-foundation` were added as direct dependencies for this and for the `AXIsProcessTrustedWithOptions` options dictionary.
- **"Asked" state** is a marker file `<root>/accessibility-asked`, written when the user picks either button. There is no settings table until stage f; this can move into `Settings` then. "Not now" uses a fifth command, `dismiss_accessibility_prompt`, so the explainer is not shown on every launch.
- **Platform UI.** `accessibility_status` reports `applies` (true on macOS); the frontend uses it for the explainer and for ⌘V vs Ctrl+V. On Windows `Frontmost::app` is `None` until stage g, so pastes are `CopiedOnly(noTarget)` with the inline "Copied — paste it where you need it" notice.
- **Permission button.** First time: the system prompt only. Already asked: opens the Accessibility pane directly. The marker is written only after one of those succeeded.
- `paste_item` always emits `history-changed` when it returns, so a vanished item also disappears from the list.

## Spec Change Log

- Review pass 1 (2026-11-13). Trigger: blind-hunter and edge-case-hunter found the "press ⌘V" notice only appeared on the next window show, so the user was not told at paste time. Amended: the frozen I/O row "Permission missing" now keeps the window open with an inline notice, then returns focus. This edit is inside the frozen block; it was made by the orchestrator on the authority of the owner brief ("fall back to 'copied, press paste yourself'"), which overrides BMad artifacts, and is flagged to the owner at the stage c checkpoint. Known-bad state avoided: silent copy-only paste with feedback deferred to a later open. KEEP: AD-7/AD-12 ordering, never paste when the target is not frontmost, Windows copy-only until stage g.

## Review Triage Log

Layers: blind-hunter, edge-case-hunter, verification-gap. One frozen-row amendment (see Spec Change Log); 11 patch entries sent to the implementer; 2 deferred; 4 rejected.

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | Item bumped before clipboard write; failed write still reorders | medium | patch | `touch` precedes `write_text` in `paste()`. |
| 2 | Own write can be captured before `record()` (race) | medium | patch | Watcher may poll between write and sequence read. |
| 3 | External copy between write and read is dropped as own | low | patch | Same window; narrowed by pending marker; residual documented. |
| 4 | Sequence reset/wrap below `last` silences capture | low | patch | `s <= last` with no distance bound. |
| 5 | macOS changeCount fallback `i64::MAX` disables capture | medium | patch | `unwrap_or(i64::MAX)` then `fetch_max`. |
| 6 | User not told at paste time when auto-paste impossible | high | patch (spec row amended) | Notice deferred to next show; brief requires the copy-only message. |
| 7 | `noTarget` shown with "press ⌘V" text though focus not returned | low | patch | Same text for all reasons. |
| 8 | Target identified by pid only (pid reuse) | medium | patch | `runningApplicationWithProcessIdentifier` without identity check. |
| 9 | Stale target kept across hides | medium | patch | `remember_frontmost` keeps previous on None; never cleared. |
| 10 | Cmd+V posted before target key window is ready | medium | patch | Posted as soon as frontmost pid matches. |
| 11 | Permission button opens system prompt and pane together; marker written before success | low | patch | `PermissionPrompt.open` calls both. |
| 12 | Global `button` styles | low | patch | Unscoped rules in App.css. |
| 13 | Platform sniffed from user agent | low | patch | `IS_MAC` from `navigator.userAgent`. |
| 14 | Missing tests: activation without permission, remember rule, timeout row, not-trusted via command | medium | patch | Verification-gap evidence: no assertions reach these paths. |
| 15 | DECISIONS entry format / spec tasks drift | low | patch | Docs; orchestrator updates the spec. |
| 16 | Paste keycode 9 is layout-dependent (Dvorak etc.) | medium | defer | Needs UCKeyTranslate lookup; listed as a known bug. |
| 17 | No frontend tests for paste/notice/prompt | medium | defer | No JS test runner (already deferred from stage b). |
| 18 | `sequence()` error after write lets own write be captured | low | reject | Worst case a source_app relabel of the pasted item; pending marker now cleared on error. |
| 19 | Windows sequence u32 wrap | low | reject | Covered by #4 distance bound; wrap needs ~4 billion changes. |
| 20 | Windows note in spec says notTrusted, code gives noTarget | low | reject | Superseded by #6/#7 redesign; Implementation Notes updated. |
| 21 | Marker file instead of settings | false | reject | Settings table arrives in stage f by design (AD-10 sequencing). |

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- all pass
- `cd src-tauri && cargo clippy --all-targets -- -D warnings` -- clean
- `npm run build` -- succeeds

**Manual checks (if no CLI):**
- Needs a human (synthetic input and the Accessibility grant cannot be automated from this terminal): with `MAGPIE_DATA_DIR=/tmp/magpie-dev npm run tauri dev`, grant Accessibility, focus TextEdit, press the hotkey, Enter → text pasted. Revoke → "press ⌘V" notice.
