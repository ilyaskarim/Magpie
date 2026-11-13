use crate::domain::TargetApp;
use crate::domain::error::CoreResult;

/// Synthetic paste into another application (CAP-5, AD-12).
pub trait Paster {
    /// Whether this platform has a paste permission the user must grant
    /// (macOS Accessibility).
    fn trust_applies(&self) -> bool;

    /// Whether Magpie may send synthetic key events (macOS Accessibility).
    fn is_trusted(&self) -> bool;

    /// Asks the OS to prompt the user for the permission, if it can. Returns
    /// whether Magpie is trusted right now (the prompt is asynchronous).
    fn request_trust(&self) -> bool;

    /// Opens the OS settings page where the permission is granted.
    fn open_trust_settings(&self) -> CoreResult<()>;

    /// Reactivates `target` and waits (about 300 ms at most) until it is
    /// frontmost and settled. `false` if it is gone, if its pid now belongs
    /// to a different app, or if it did not come to the front in time.
    fn activate(&self, target: &TargetApp) -> bool;

    /// Sends one Cmd+V (macOS) / Ctrl+V (Windows) to the frontmost app.
    fn send_paste(&self) -> CoreResult<()>;
}

#[cfg(test)]
pub mod fakes {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::domain::CoreError;

    /// Records what it was asked to do.
    #[derive(Default)]
    pub struct FakePaster {
        pub applies: bool,
        pub trusted: bool,
        /// Whether `activate` succeeds.
        pub activates: bool,
        pub paste_fails: bool,
        pub settings_fail: bool,
        pub activated: RefCell<Vec<i64>>,
        pub pastes: Cell<usize>,
        pub trust_requests: Cell<usize>,
    }

    impl FakePaster {
        pub fn granted() -> Self {
            Self {
                trusted: true,
                activates: true,
                ..Self::default()
            }
        }
    }

    impl Paster for FakePaster {
        fn trust_applies(&self) -> bool {
            self.applies
        }

        fn is_trusted(&self) -> bool {
            self.trusted
        }

        fn request_trust(&self) -> bool {
            self.trust_requests.set(self.trust_requests.get() + 1);
            self.trusted
        }

        fn open_trust_settings(&self) -> CoreResult<()> {
            if self.settings_fail {
                return Err(CoreError::Paste("fake failure".into()));
            }
            Ok(())
        }

        fn activate(&self, target: &TargetApp) -> bool {
            self.activated.borrow_mut().push(target.pid);
            self.activates
        }

        fn send_paste(&self) -> CoreResult<()> {
            if self.paste_fails {
                return Err(CoreError::Paste("fake failure".into()));
            }
            self.pastes.set(self.pastes.get() + 1);
            Ok(())
        }
    }
}
