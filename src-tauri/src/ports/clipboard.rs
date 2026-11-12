use crate::domain::error::CoreResult;

/// Read access to the system clipboard.
pub trait ClipboardReader {
    /// The raw names of the formats currently on the clipboard. This is read
    /// before any content, so the privacy filter (AD-6) can run on it.
    fn formats(&self) -> CoreResult<Vec<String>>;

    /// The plain-text representation, or `None` if the clipboard has none.
    fn text(&self) -> CoreResult<Option<String>>;
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
    use super::*;
    use crate::domain::CoreError;

    /// An in-memory clipboard.
    #[derive(Default)]
    pub struct FakeClipboard {
        pub formats: Vec<String>,
        pub text: Option<String>,
        pub fail: bool,
    }

    impl FakeClipboard {
        pub fn with_text(text: &str) -> Self {
            Self {
                formats: vec!["public.utf8-plain-text".into()],
                text: Some(text.into()),
                fail: false,
            }
        }

        pub fn image_only() -> Self {
            Self {
                formats: vec!["public.png".into()],
                text: None,
                fail: false,
            }
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
