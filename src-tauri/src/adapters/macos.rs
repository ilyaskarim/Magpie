//! macOS: frontmost application via `NSWorkspace`, Accessibility trust,
//! cooperative app activation and the synthetic Cmd+V (`CGEvent`).

use std::time::{Duration, Instant};

use objc2::rc::autoreleasepool;
use objc2::runtime::NSObjectProtocol;
use objc2::sel;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSRunningApplication, NSWorkspace,
};
use objc2_application_services::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
};
use objc2_foundation::{NSString, NSURL};

use crate::domain::error::CoreResult;
use crate::domain::{CoreError, TargetApp};
use crate::ports::{Frontmost, Paster};

/// How long to wait for the saved app to become frontmost (AD-12).
const ACTIVATION_TIMEOUT: Duration = Duration::from_millis(300);
const ACTIVATION_POLL: Duration = Duration::from_millis(10);

/// Time for the reactivated app's key window to take focus before Cmd+V.
const SETTLE_DELAY: Duration = Duration::from_millis(60);

/// `kVK_ANSI_V`.
const KEY_V: u16 = 9;

/// The Privacy & Security > Accessibility pane. A local URL handled by the
/// OS; nothing goes over the network.
const ACCESSIBILITY_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

fn own_pid() -> i64 {
    i64::from(std::process::id())
}

/// The frontmost app's pid, read fresh.
fn frontmost_pid() -> Option<i64> {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| i64::from(app.processIdentifier()))
    })
}

pub struct SystemFrontmost;

impl Frontmost for SystemFrontmost {
    fn app_name(&self) -> Option<String> {
        self.app().and_then(|a| a.name)
    }

    fn app(&self) -> Option<TargetApp> {
        // Called from the long-lived watcher thread, which has no
        // autorelease pool of its own.
        autoreleasepool(|_| {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            let pid = i64::from(app.processIdentifier());
            // Magpie itself is never a copy source or a paste target.
            if pid == own_pid() {
                return None;
            }
            let name = app
                .localizedName()
                .map(|n| n.to_string())
                .filter(|n| !n.is_empty());
            let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
            Some(TargetApp {
                pid,
                name,
                bundle_id,
            })
        })
    }
}

pub struct SystemPaster;

impl Paster for SystemPaster {
    fn trust_applies(&self) -> bool {
        true
    }

    fn is_trusted(&self) -> bool {
        // SAFETY: no arguments, no preconditions.
        unsafe { AXIsProcessTrusted() }
    }

    fn request_trust(&self) -> bool {
        // SAFETY: reading an immutable framework constant.
        let key: &CFString = unsafe { kAXTrustedCheckOptionPrompt };
        let options =
            CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[CFBoolean::new(true)]);
        // SAFETY: the dictionary maps the documented key to a CFBoolean.
        unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
    }

    fn open_trust_settings(&self) -> CoreResult<()> {
        autoreleasepool(|_| {
            let url =
                NSURL::URLWithString(&NSString::from_str(ACCESSIBILITY_PANE)).ok_or_else(|| {
                    CoreError::Paste("the Accessibility settings address is invalid".into())
                })?;
            if NSWorkspace::sharedWorkspace().openURL(&url) {
                Ok(())
            } else {
                Err(CoreError::Paste(
                    "could not open System Settings > Privacy & Security > Accessibility".into(),
                ))
            }
        })
    }

    fn activate(&self, saved: &TargetApp) -> bool {
        let Ok(pid) = i32::try_from(saved.pid) else {
            return false;
        };
        let bundle_id = saved.bundle_id.clone();
        // NSApplication is main-thread only; commands run on a worker.
        let requested = dispatch2::run_on_main(move |mtm| {
            autoreleasepool(|_| {
                let Some(target) =
                    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
                else {
                    return false;
                };
                if target.isTerminated() {
                    return false;
                }
                // A reused pid belongs to a different app: never paste there.
                if target.bundleIdentifier().map(|b| b.to_string()) != bundle_id {
                    return false;
                }
                // macOS 14+: cooperative activation. `ActivateIgnoringOtherApps`
                // has no effect any more, so Magpie yields first (AD-12).
                let app = NSApplication::sharedApplication(mtm);
                if app.respondsToSelector(sel!(yieldActivationToApplication:)) {
                    app.yieldActivationToApplication(&target);
                }
                target.activateWithOptions(NSApplicationActivationOptions::empty())
            })
        });
        if !requested {
            return false;
        }
        // Wait off the main thread so AppKit can process the switch.
        let pid = i64::from(pid);
        let deadline = Instant::now() + ACTIVATION_TIMEOUT;
        loop {
            if frontmost_pid() == Some(pid) {
                std::thread::sleep(SETTLE_DELAY);
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(ACTIVATION_POLL);
        }
    }

    fn send_paste(&self) -> CoreResult<()> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState);
        let key = |down: bool| {
            CGEvent::new_keyboard_event(source.as_deref(), KEY_V, down)
                .ok_or_else(|| CoreError::Paste("could not create the Cmd+V key event".into()))
        };
        let down = key(true)?;
        let up = key(false)?;
        for event in [&down, &up] {
            CGEvent::set_flags(Some(event), CGEventFlags::MaskCommand);
        }
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&down));
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&up));
        Ok(())
    }
}
