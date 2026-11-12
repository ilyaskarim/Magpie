# Build stages

Order fixed by the owner brief. Each stage works end to end before the next starts, lives on its own branch, and merges to `main` when done.

| Stage | Scope | Capabilities | Owner checkpoint |
| --- | --- | --- | --- |
| a | Scaffold, tray icon, hotkey opens empty window (done, on main) | CAP-3 (window shell), CAP-9 (tray shell) | Yes, with BMad setup |
| b | Capture text into SQLite and list it | CAP-1 (text), CAP-3 (list) | |
| c | Paste back into the previous app, Accessibility flow, fallback | CAP-5 | Yes: demo paste working |
| d | Search, dedupe, pins | CAP-4, CAP-2, CAP-6 | |
| e | Images, HTML, files | CAP-1 (rest), CAP-3 (previews), CAP-8 (image files on disk) | |
| f | Privacy filters, retention, settings screen | CAP-7, CAP-8, CAP-10, CAP-9 (pause, settings, launch at login) | |
| g | Second platform pass (Windows) and polish | All, on Windows | |

Tests required by the brief land with the stage that introduces the logic: dedupe (d), search (d), retention (f), privacy filters (f).
