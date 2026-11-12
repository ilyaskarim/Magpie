# Review — Currency / reality check of ARCHITECTURE-SPINE.md

Reviewer lens: every committed decision verified against the web (crates.io, npm), the published crate sources, or the existing project (Cargo.toml, Cargo.lock, package.json, node_modules). Checked 2026-11-13.

**Verdict:** Currency is strong. Every pinned version matches the latest stable release and the lockfile, and the clipboard-rs and FTS5 claims hold against the shipped source. The gaps are versions the spine leaves open and native-API choices it never pins. One of those choices (reactivating the saved app on macOS 14+) has a behavioural change that the spine does not account for.

## 1. Stack table vs. registries vs. project

| Item | Spine | Latest stable (registry, 2026-11-13) | Project (lock / installed) | Status |
| --- | --- | --- | --- | --- |
| tauri | 2.12.5 | 2.12.5 (2026-11-06) | Cargo.lock 2.12.5; @tauri-apps/api & cli 2.12.4 | OK |
| tauri-plugin-global-shortcut | 2.4.1 | 2.4.1 | 2.4.1 | OK |
| tauri-plugin-single-instance | 2.5.5 | 2.5.5 | 2.5.5 | OK |
| tauri-plugin-autostart | "2.x (stage f)" | **2.7.0** (crate) / @tauri-apps/plugin-autostart 2.7.0 | not yet added | Unpinned (finding C-3) |
| rusqlite | 0.42.0 | 0.42.0 (released 2026-11-11, 2 days old) → libsqlite3-sys 0.40.0, SQLite 3.54.1 | not yet added | OK, very fresh; MSRV 1.89 |
| clipboard-rs | 0.3.5 | 0.3.5 (2026-06-30) | not yet added | OK |
| React | 19 | 19.3.1 | 19.3.1 | OK |
| TypeScript | 6 | **7.0.2** | ~6.0.3 | Deliberately behind; not stated (C-5) |
| Vite | 8 | 8.3.8 | 8.3.8 | OK |
| core-graphics (stack.md) | unversioned | 0.25.0 (2025-05) | 0.25.0 in lock (via tao) | Unpinned, legacy binding (C-2) |
| windows (stack.md) | unversioned | 0.62.2 | 0.62.2 in lock (via tauri, wry, tao) | Unpinned (C-4) |

Rust edition 2024 / toolchain 1.90+ in stack.md works for rusqlite 0.42 (MSRV 1.89), autostart 2.7 (MSRV 1.90) and clipboard-rs (MSRV 1.67).

## 2. Claims verified against the source

- **rusqlite `bundled` includes FTS5. Confirmed.** libsqlite3-sys 0.40.0 `build.rs` compiles the bundled amalgamation with `-DSQLITE_ENABLE_FTS5` (and FTS3, JSON1). Plain `bundled` is enough; `bundled-full` is not needed.
- **`unicode61 remove_diacritics 2`. Valid.** The FTS5 unicode61 tokenizer accepts `remove_diacritics` values 0/1/2 (`FTS5_REMOVE_DIACRITICS_COMPLEX` = 2) in SQLite 3.54.1. In DDL it must be written as `tokenize = 'unicode61 remove_diacritics 2'` (space-separated FTS5 syntax, not the FTS3/4 `=` form). Suggest putting that exact string in AD-8 so no one writes the FTS4 form.
- **clipboard-rs 0.3.5:**
  - Configurable macOS watch interval. **Confirmed:** `ClipboardWatcherContext::new_with_interval(Duration)`, default 500 ms, polls `NSPasteboard.changeCount`. On Windows the interval is ignored and changes come through `clipboard_win::Monitor` (`AddClipboardFormatListener` / `WM_CLIPBOARDUPDATE`), which matches stack.md.
  - Available formats list. **Confirmed:** `available_formats() -> Vec<String>`. On macOS it returns the raw `NSPasteboard.types` UTIs, so `org.nspasteboard.ConcealedType` and `TransientType` appear as-is. On Windows it returns `format_name_big` names, so registered formats such as `ExcludeClipboardContentFromMonitorProcessing` and `CanIncludeInClipboardHistory` appear by name.
  - Custom format buffers. **Confirmed:** `get_buffer(&str)` and `set_buffer(&str, Vec<u8>)`.
  - Image read as RGBA. **Confirmed with a caveat:** `get_image() -> RustImageData` (PNG first, then TIFF via NSImage on macOS), then `to_rgba8()`. This needs the default `image` feature, so do not set `default-features = false`.
  - File list. **Confirmed:** `get_files() -> Vec<String>`. On macOS it reads file `NSURL`s and returns POSIX paths.
  - Handler signal. `ClipboardHandler::on_clipboard_change(&mut self)` carries no payload or change count. The capture loop has to query formats and the frontmost app itself, which fits AD-6.
- **AD-6 detail:** `CanIncludeInClipboardHistory = 0` cannot be decided from the format list alone. Its value is a 4-byte DWORD that has to be read with `get_buffer`. AD-6 says "before any payload is read". Reword it to "before any content payload is read; marker-format values may be read".
- **NSWorkspace.frontmostApplication, NSRunningApplication:** available in objc2-app-kit 0.3.2, which is already in the lockfile through tauri, tao, muda and tray-icon.
- **AXIsProcessTrustedWithOptions:** available in `objc2-application-services` 0.3.2 (HIServices). Not yet in the lock.
- **CGEvent post:** available both in `core-graphics` 0.25 (servo core-foundation-rs) and in `objc2-core-graphics` 0.3.2 (`CGEvent::post`). The latter is already in the lock through tray-icon and softbuffer.
- **SendInput / GetClipboardOwner / QueryFullProcessImageNameW:** in the `windows` crate 0.62 behind feature flags (`Win32_UI_Input_KeyboardAndMouse`, `Win32_System_DataExchange`, `Win32_System_Threading`, `Win32_UI_WindowsAndMessaging`).

## 3. Findings

### C-1 (High): macOS 14+ app reactivation is not addressed (AD-12)
AD-12 says to "reactivate the saved app" without naming the API. In objc2-app-kit 0.3.2, `NSApplicationActivateIgnoringOtherApps` is marked *"deprecated in macOS 14 and will have no effect"*, and `NSApp.activateIgnoringOtherApps` is deprecated. macOS 14 introduced cooperative activation (`NSApp.yieldActivationToApplication(_:)` followed by `NSRunningApplication.activateFromApplication_options`). A background or accessory app calling `activateWithOptions` may not take focus back, so the Cmd+V would go to the wrong place or be dropped.
**Fix:** In AD-12, specify: hide Magpie's window, call `NSApp.yieldActivationToApplication(saved)`, then `saved.activateWithOptions(NSApplicationActivateAllWindows)` (or `activateFromApplication_options` on 14+). Before posting Cmd+V, poll `frontmostApplication` until it equals the saved PID or a timeout (about 300 ms) is reached; replace the fixed "short settle delay" with this. Make this an explicit spike in stage e.

### C-2 (Medium): native bindings for macOS are not chosen and duplicate existing ones
stack.md names `core-graphics` (servo, last release 2025-05). Tauri's own tree is already on the objc2 family (`objc2-app-kit`, `objc2-core-graphics` 0.3.2). Mixing the two bindings brings in two CoreFoundation type systems.
**Fix:** Commit to the objc2 family: `objc2-app-kit` 0.3 (features `NSWorkspace`, `NSRunningApplication`), `objc2-core-graphics` 0.3 (`CGEvent`), and `objc2-application-services` 0.3 (`AXIsProcessTrustedWithOptions`). Add those versions to the Stack table.

### C-3 (Medium): autostart is left as "2.x"
The current version is 2.7.0 for both the crate and npm. Its macOS default is `MacosLauncher::LaunchAgent`, which writes a plist to `~/Library/LaunchAgents`. The alternative is `AppleScript`, which triggers an Automation permission prompt. There is no `SMAppService` option.
**Fix:** Pin `tauri-plugin-autostart = "2.7"` and `@tauri-apps/plugin-autostart` 2.7. Record `MacosLauncher::LaunchAgent` as the decision, and note that macOS shows a "Background Items Added" notice.

### C-4 (Low): the `windows` crate is unpinned
**Fix:** Pin `windows = "0.62"` to match the version Tauri already resolves (0.62.2), so the tree has one copy, and list the four feature flags.

### C-5 (Low): the TypeScript major is behind latest without a stated reason
npm latest is 7.0.2; the project uses ~6.0.3. That is a reasonable choice, since TS 7 is the native-compiler line and Vite and plugin compatibility should be checked first, but the spine does not say it is deliberate.
**Fix:** Add a note to the Stack row: "TS 6.0 (7.x deferred until toolchain compatibility is checked)".

### C-6 (Low): rusqlite 0.42.0 is 2 days old
It is the correct latest version. Pin it exactly (`=0.42.0` or a committed lock) and run the FTS5 tokenizer DDL in a migration test in stage b. That way a regression from the new SQLite 3.54.1 build would show up early.

### Note: clipboard-rs Windows watcher panics on failure
`start_watch` calls `Monitor::new().expect(...)`. A failure there kills the watcher thread. Run the watcher under `catch_unwind` or with a supervisor that restarts it, in line with the spine's no-unwrap rule for OS paths.
