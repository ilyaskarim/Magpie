//! Concrete implementations of the ports. All `#[cfg(target_os)]` code lives
//! here.

pub mod clipboard;
pub mod sqlite;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{SystemFrontmost, SystemPaster};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{SystemFrontmost, SystemPaster};

#[cfg(not(any(target_os = "macos", windows)))]
mod other {
    use crate::domain::error::CoreResult;
    use crate::domain::{CoreError, TargetApp};
    use crate::ports::{Frontmost, Paster};

    /// Frontmost-app lookup is not implemented on this platform.
    pub struct SystemFrontmost;

    impl Frontmost for SystemFrontmost {
        fn app_name(&self) -> Option<String> {
            None
        }

        fn app(&self) -> Option<TargetApp> {
            None
        }
    }

    /// Synthetic paste is not implemented on this platform.
    pub struct SystemPaster;

    impl Paster for SystemPaster {
        fn trust_applies(&self) -> bool {
            false
        }

        fn is_trusted(&self) -> bool {
            false
        }

        fn request_trust(&self) -> bool {
            false
        }

        fn open_trust_settings(&self) -> CoreResult<()> {
            Err(CoreError::Paste("not available on this platform".into()))
        }

        fn activate(&self, _target: &TargetApp) -> bool {
            false
        }

        fn send_paste(&self) -> CoreResult<()> {
            Err(CoreError::Paste("not available on this platform".into()))
        }
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
pub use other::{SystemFrontmost, SystemPaster};
