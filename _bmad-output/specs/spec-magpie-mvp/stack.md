# Stack

Fixed by the owner brief; versions checked against official docs and registries on 2026-11-13.

| Layer | Choice | Version |
| --- | --- | --- |
| Shell | Tauri | 2.12.x (`tauri` crate), `@tauri-apps/cli` 2.12.x |
| Backend | Rust, edition 2024 | toolchain 1.90+ |
| Frontend | React + TypeScript + Vite | React 19, TS 6, Vite 8 |
| Storage | SQLite via `rusqlite` with `bundled` (FTS5 compiled in) | =0.42.0 |
| Clipboard read/write + watch | `clipboard-rs` (MIT) | 0.3.5 |
| Hotkey | `tauri-plugin-global-shortcut` (official) | 2.4 |
| Single instance | `tauri-plugin-single-instance` (official) | 2.5 |
| Launch at login | `tauri-plugin-autostart` (official), `MacosLauncher::LaunchAgent` | 2.7 |
| macOS native calls | `objc2-app-kit`, `objc2-core-graphics`, `objc2-application-services` (same family Tauri already uses) | 0.3 |
| Windows native calls | `windows` | 0.62 |

## Clipboard monitoring

| OS | Mechanism | Polls? |
| --- | --- | --- |
| macOS | Read `NSPasteboard.changeCount` every 500 ms; read contents only when it changes | Yes: macOS has no change notification |
| Windows | `AddClipboardFormatListener` / `WM_CLIPBOARDUPDATE` via `clipboard-rs` | No |

Rejected: `tauri-plugin-clipboard` (no commits since 2024-10, hides the format list), `tauri-plugin-clipboard-x` (pins clipboard-rs 0.2, no format list). Both wrap the same crate, so they would not reduce polling.

## Source app

| OS | How |
| --- | --- |
| macOS | `NSWorkspace.frontmostApplication` at change time |
| Windows | `GetClipboardOwner` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`; fall back to `GetForegroundWindow` when the owner is NULL |

## Synthetic paste

Direct `CGEvent` post (macOS, `objc2-core-graphics`) and `SendInput` (Windows, `windows` crate) for a single Cmd/Ctrl+V. Accessibility check on macOS via `AXIsProcessTrustedWithOptions` (`objc2-application-services`). Previous-app reactivation follows architecture AD-12.
