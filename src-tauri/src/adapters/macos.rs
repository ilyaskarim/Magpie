//! macOS frontmost application via `NSWorkspace`.

use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;

use crate::ports::Frontmost;

pub struct SystemFrontmost;

impl Frontmost for SystemFrontmost {
    fn app_name(&self) -> Option<String> {
        // Called from the long-lived watcher thread, which has no
        // autorelease pool of its own.
        autoreleasepool(|_| {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            // Magpie itself is never the source of a capture.
            if i64::from(app.processIdentifier()) == i64::from(std::process::id()) {
                return None;
            }
            let name = app.localizedName()?.to_string();
            (!name.is_empty()).then_some(name)
        })
    }
}
