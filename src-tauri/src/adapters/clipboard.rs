//! The system clipboard through `clipboard-rs`. On macOS the watcher polls
//! `NSPasteboard.changeCount` every 500 ms (the crate's default); on Windows
//! it listens for `WM_CLIPBOARDUPDATE`.

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat,
};

use crate::domain::CoreError;
use crate::domain::error::CoreResult;
use crate::ports::{ClipboardReader, ClipboardWatch};

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
