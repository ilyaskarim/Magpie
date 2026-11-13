//! The system clipboard through `clipboard-rs`. On macOS the watcher polls
//! `NSPasteboard.changeCount` every 500 ms (the crate's default); on Windows
//! it listens for `WM_CLIPBOARDUPDATE`.

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat,
};

use crate::domain::CoreError;
use crate::domain::error::CoreResult;
use crate::ports::{ClipboardReader, ClipboardSequence, ClipboardWatch, ClipboardWriter};

fn clip_err(context: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::Clipboard(format!("{context}: {e}"))
}

pub struct SystemClipboard {
    ctx: ClipboardContext,
}

impl SystemClipboard {
    pub fn new() -> CoreResult<Self> {
        let ctx =
            ClipboardContext::new().map_err(|e| clip_err("could not open the clipboard", e))?;
        Ok(Self { ctx })
    }
}

impl ClipboardReader for SystemClipboard {
    fn formats(&self) -> CoreResult<Vec<String>> {
        self.ctx
            .available_formats()
            .map_err(|e| clip_err("could not list formats", e))
    }

    fn text(&self) -> CoreResult<Option<String>> {
        if !self.ctx.has(ContentFormat::Text) {
            return Ok(None);
        }
        self.ctx
            .get_text()
            .map(Some)
            .map_err(|e| clip_err("could not read text", e))
    }
}

impl ClipboardSequence for SystemClipboard {
    #[cfg(target_os = "macos")]
    fn sequence(&self) -> CoreResult<Option<i64>> {
        use objc2::rc::autoreleasepool;
        use objc2_app_kit::NSPasteboard;
        // Also called from the watcher thread, which has no autorelease pool.
        let count = autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount());
        // Out of range is impossible in practice; unknown is the safe answer
        // (it is never treated as an own write).
        Ok(i64::try_from(count).ok())
    }

    #[cfg(windows)]
    fn sequence(&self) -> CoreResult<Option<i64>> {
        // SAFETY: no arguments; returns 0 when the window station has no
        // clipboard access, which we treat as unknown.
        let n = unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() };
        Ok((n != 0).then_some(i64::from(n)))
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    fn sequence(&self) -> CoreResult<Option<i64>> {
        Ok(None)
    }
}

impl ClipboardWriter for SystemClipboard {
    fn write_text(&self, text: &str) -> CoreResult<()> {
        self.ctx
            .set_text(text.to_string())
            .map_err(|e| clip_err("could not write text", e))
    }
}

/// Watches the system clipboard for changes.
#[derive(Default)]
pub struct SystemWatcher;

struct Handler<'a>(&'a mut dyn FnMut());

impl ClipboardHandler for Handler<'_> {
    fn on_clipboard_change(&mut self) {
        (self.0)();
    }
}

impl ClipboardWatch for SystemWatcher {
    fn watch(&mut self, on_change: &mut dyn FnMut()) -> CoreResult<()> {
        let mut ctx = ClipboardWatcherContext::new()
            .map_err(|e| clip_err("could not start the clipboard watcher", e))?;
        ctx.add_handler(Handler(on_change));
        ctx.start_watch();
        Ok(())
    }
}
