//! Windows frontmost application. The clipboard-owner lookup arrives in
//! stage g; until then the source app is unknown.

use crate::ports::Frontmost;

pub struct SystemFrontmost;

impl Frontmost for SystemFrontmost {
    fn app_name(&self) -> Option<String> {
        None
    }
}
