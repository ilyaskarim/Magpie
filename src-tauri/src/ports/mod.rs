//! Traits the services depend on; adapters implement them (AD-1).

pub mod clipboard;
pub mod frontmost;
pub mod store;

pub use clipboard::{ClipboardReader, ClipboardWatch};
pub use frontmost::Frontmost;
pub use store::Store;
