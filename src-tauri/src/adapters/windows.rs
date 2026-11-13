//! Windows frontmost application and synthetic paste. The clipboard-owner
//! lookup, `SetForegroundWindow` and `SendInput` arrive in stage g; until
//! then the source app is unknown and paste-back copies only.

use crate::domain::error::CoreResult;
use crate::domain::{CoreError, TargetApp};
use crate::ports::{Frontmost, Paster};

pub struct SystemFrontmost;

impl Frontmost for SystemFrontmost {
    fn app_name(&self) -> Option<String> {
        None
    }

    fn app(&self) -> Option<TargetApp> {
        None
    }
}

/// Not implemented yet: every paste-back is `CopiedOnly`.
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
        Err(CoreError::Paste(
            "automatic paste is not available on Windows yet".into(),
        ))
    }

    fn activate(&self, _target: &TargetApp) -> bool {
        false
    }

    fn send_paste(&self) -> CoreResult<()> {
        Err(CoreError::Paste(
            "automatic paste is not available on Windows yet".into(),
        ))
    }
}
