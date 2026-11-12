use crate::domain::error::CoreResult;

/// Read access to the system clipboard.
pub trait ClipboardReader {
    /// The raw names of the formats currently on the clipboard. This is read
    /// before any content, so the privacy filter (AD-6) can run on it.
    fn formats(&self) -> CoreResult<Vec<String>>;

    /// The plain-text representation, or `None` if the clipboard has none.
    fn text(&self) -> CoreResult<Option<String>>;
}

/// The OS clipboard sequence number (AD-7): macOS `NSPasteboard.changeCount`,
/// Windows `GetClipboardSequenceNumber`. It grows with every clipboard change.
pub trait ClipboardSequence {
    /// The current sequence number, or `None` where the platform has none
    /// (then no change is ever treated as Magpie's own write).
    fn sequence(&self) -> CoreResult<Option<i64>>;
}

/// Write access to the system clipboard, for paste-back (CAP-5).
pub trait ClipboardWriter: ClipboardSequence {
    /// Replaces the clipboard contents with `text` as plain text (AD-13: for
    /// a text-only item that is every stored representation).
    fn write_text(&self, text: &str) -> CoreResult<()>;
}

/// A source of clipboard-change notifications.
pub trait ClipboardWatch {
    /// Blocks, calling `on_change` after every clipboard change, until the
    /// watcher stops. Content already on the clipboard when watching starts
    /// does not count as a change (AD-18).
    fn watch(&mut self, on_change: &mut dyn FnMut()) -> CoreResult<()>;
}

#[cfg(test)]
pub mod fakes {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::domain::CoreError;

    /// An in-memory clipboard. Every write bumps `seq` by one, like the OS.
    #[derive(Default)]
    pub struct FakeClipboard {
        pub formats: Vec<String>,
        pub text: Option<String>,
        pub fail: bool,
        pub seq: Cell<i64>,
        /// Everything written, in order.
        pub writes: RefCell<Vec<String>>,
    }

    impl FakeClipboard {
        pub fn with_text(text: &str) -> Self {
            Self {
                formats: vec!["public.utf8-plain-text".into()],
                text: Some(text.into()),
                ..Self::default()
            }
        }

        pub fn image_only() -> Self {
            Self {
                formats: vec!["public.png".into()],
                ..Self::default()
            }
        }

        /// Simulates a copy made by another app.
        pub fn external_change(&self) {
            self.seq.set(self.seq.get() + 1);
        }
    }

    impl ClipboardReader for FakeClipboard {
        fn formats(&self) -> CoreResult<Vec<String>> {
            if self.fail {
                return Err(CoreError::Clipboard("fake failure".into()));
            }
            Ok(self.formats.clone())
        }

        fn text(&self) -> CoreResult<Option<String>> {
            if self.fail {
                return Err(CoreError::Clipboard("fake failure".into()));
            }
            Ok(self.text.clone())
        }
    }

    impl ClipboardSequence for FakeClipboard {
        fn sequence(&self) -> CoreResult<Option<i64>> {
            Ok(Some(self.seq.get()))
        }
    }

    impl ClipboardWriter for FakeClipboard {
        fn write_text(&self, text: &str) -> CoreResult<()> {
            if self.fail {
                return Err(CoreError::Clipboard("fake failure".into()));
            }
            self.writes.borrow_mut().push(text.to_string());
            self.seq.set(self.seq.get() + 1);
            Ok(())
        }
    }

    /// Fires `changes` notifications, then returns.
    pub struct FakeWatch {
        pub changes: usize,
    }

    impl ClipboardWatch for FakeWatch {
        fn watch(&mut self, on_change: &mut dyn FnMut()) -> CoreResult<()> {
            for _ in 0..self.changes {
                on_change();
            }
            Ok(())
        }
    }
}
