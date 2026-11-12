//! Concrete implementations of the ports. All `#[cfg(target_os)]` code lives
//! here.

pub mod clipboard;
pub mod sqlite;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::SystemFrontmost;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::SystemFrontmost;

#[cfg(not(any(target_os = "macos", windows)))]
mod other {
    use crate::ports::Frontmost;

    /// Frontmost-app lookup is not implemented on this platform.
    pub struct SystemFrontmost;

    impl Frontmost for SystemFrontmost {
        fn app_name(&self) -> Option<String> {
            None
        }
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
pub use other::SystemFrontmost;
