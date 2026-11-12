# Magpie

A local-first, cross-platform clipboard manager. Magpie records what you
copy, keeps it searchable, and pastes any past item into the app you're
working in. No account, no telemetry, no network calls: your history never
leaves your machine.

Status: early development. Windows and macOS first; Linux later.

## Requirements

| Tool | Version |
| --- | --- |
| Rust (via [rustup](https://rustup.rs)) | stable, 1.90 or newer |
| Node.js | 20.12 or newer |
| npm | comes with Node |

Platform prerequisites for Tauri 2 are listed at
<https://v2.tauri.app/start/prerequisites/>. In short:

- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** Microsoft C++ Build Tools ("Desktop development with C++")
  and WebView2 (preinstalled on Windows 10 1803+ and Windows 11).

## Setup

```sh
git clone https://github.com/ilyaskarim/Magpie.git
cd Magpie
npm install
```

## Run (development)

```sh
npm run tauri dev
```

Magpie runs in the background. It has no Dock icon on macOS; look for it
in the menu bar (macOS) or the system tray (Windows).

- **Open the history window:** <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>V</kbd>
  on macOS, <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>V</kbd> on Windows, or
  "Open Magpie" from the tray menu.
- **Dismiss it:** <kbd>Esc</kbd>, or click anywhere else.
- **Quit:** "Quit Magpie" in the tray menu.

To keep test runs away from your real history, a debug build can use a
throwaway data folder (ignored by release builds):

```sh
MAGPIE_DATA_DIR=/tmp/magpie-dev npm run tauri dev
```

History is stored in SQLite under the app's local data folder
(`~/Library/Application Support/app.magpie.desktop/` on macOS,
`%LOCALAPPDATA%\app.magpie.desktop\` on Windows).

## Build

```sh
npm run tauri build
```

The installer or app bundle is written to `src-tauri/target/release/bundle/`.

## Tests

```sh
cd src-tauri
cargo test
```

## BMad Method

Planning and stories are run with the [BMad Method](https://docs.bmad-method.org/)
(v6.12.4, `bmm` module), set up for Claude Code. The installed skills live
in `.claude/skills/`, BMad's runtime and shared config in `_bmad/`, and
planning artifacts in `_bmad-output/`.

Per-user install answers (your name, skill level) are not committed. After
cloning, run this once to create them; it needs Node.js 20.12+, Python
3.10+ and [uv](https://docs.astral.sh/uv/):

```sh
npx bmad-method@6.12.4 install --directory . --modules bmm --tools claude-code --yes
```

Then start Claude Code in the repo and run `/bmad-help`.

## Renaming the app

The user-facing name is `productName` in `src-tauri/tauri.conf.json`.
Rust reads it through `app_name()` and the frontend through
`getName()`. The crate name (`magpie`), npm package name and bundle
identifier (`app.magpie.desktop`) are internal identifiers. Change them
only if you also want a new identity on disk, because the identifier
decides where user data is stored.
