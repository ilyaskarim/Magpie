//! Traits the services depend on; adapters implement them (AD-1).

pub mod clipboard;
pub mod frontmost;
pub mod paster;
pub mod store;

pub use clipboard::{ClipboardReader, ClipboardSequence, ClipboardWatch, ClipboardWriter};
pub use frontmost::Frontmost;
pub use paster::Paster;
pub use store::Store;
