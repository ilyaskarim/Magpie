//! Paste-back (CAP-5): put a past item on the clipboard and paste it into the
//! app the user was working in (AD-7, AD-12, AD-13).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use super::capture::lock;
use crate::domain::TargetApp;
use crate::domain::error::CoreResult;
use crate::ports::{ClipboardWriter, Paster, Store};

/// Magpie's own clipboard writes (AD-7), shared by the paste command and the
/// watcher thread. While a write is in flight every change counts as own;
/// afterwards, changes at or just below the recorded sequence number do.
#[derive(Debug)]
pub struct OwnWrites {
    pending: AtomicBool,
    last: AtomicI64,
}

/// "No own write yet".
const NONE: i64 = i64::MIN;

/// How far below the recorded sequence number a change still counts as own.
/// Anything further is a sequence reset or wrap, not one of our writes, and
/// must not silence capture.
const OWN_WINDOW: i64 = 64;

impl Default for OwnWrites {
    fn default() -> Self {
        Self {
            pending: AtomicBool::new(false),
            last: AtomicI64::new(NONE),
        }
    }
}

impl OwnWrites {
    /// Call right before writing the clipboard: until [`record`] or
    /// [`cancel`], the watcher treats every change as Magpie's own.
    ///
    /// [`record`]: Self::record
    /// [`cancel`]: Self::cancel
    pub fn begin(&self) {
        self.pending.store(true, Ordering::SeqCst);
    }

    /// Records the sequence number read right after the write and ends the
    /// pending state.
    pub fn record(&self, seq: Option<i64>) {
        if let Some(seq) = seq {
            self.last.store(seq, Ordering::SeqCst);
        }
        self.pending.store(false, Ordering::SeqCst);
    }

    /// Ends the pending state without recording (the write failed, or its
    /// sequence number could not be read).
    pub fn cancel(&self) {
        self.pending.store(false, Ordering::SeqCst);
    }

    /// Whether a clipboard state with this sequence number was written by
    /// Magpie. Outside a pending write, an unknown sequence number is never
    /// an own write.
    pub fn is_own(&self, seq: Option<i64>) -> bool {
        if self.pending.load(Ordering::SeqCst) {
            return true;
        }
        let last = self.last.load(Ordering::SeqCst);
        matches!(seq, Some(s) if last != NONE && s <= last && last - s < OWN_WINDOW)
    }
}

/// Why the item was only copied, not pasted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopiedOnlyReason {
    /// Synthetic input is not allowed (macOS Accessibility not granted, or
    /// not implemented on this platform yet). The window was not hidden.
    NotTrusted,
    /// No app was recorded when the window opened. The window was not hidden.
    NoTarget,
    /// The saved app quit, or did not come to the front in time. Never paste
    /// into whatever else is frontmost. The window was hidden.
    TargetUnavailable,
    /// The keystroke could not be sent. The window was hidden.
    PasteFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Pasted,
    CopiedOnly(CopiedOnlyReason),
}

/// Pastes item `item_id` into `target`:
///
/// 1. read the item (a missing item is an error, nothing else happens and
///    the window stays open);
/// 2. write the clipboard, marking the write as Magpie's own (AD-7);
/// 3. only after a successful write, move the item to the top
///    (`last_copied_at = now`, `source_app` unchanged);
/// 4. without a target or without the permission, stop here with the window
///    still open, so the UI can say "copied, press paste yourself";
/// 5. otherwise `hide` the window, reactivate the saved app, wait until it
///    is frontmost and send Cmd/Ctrl+V (AD-12).
///
/// Every outcome after step 2 leaves the item on the clipboard.
#[allow(clippy::too_many_arguments)]
pub fn paste<S, C, P>(
    item_id: &str,
    target: Option<&TargetApp>,
    store: &Mutex<S>,
    clipboard: &C,
    paster: &P,
    own_seq: &OwnWrites,
    now: i64,
    hide: &dyn Fn(),
) -> CoreResult<PasteOutcome>
where
    S: Store + ?Sized,
    C: ClipboardWriter + ?Sized,
    P: Paster + ?Sized,
{
    // One store operation per lock (AD-11).
    let text = lock(store).get_text(item_id)?;

    own_seq.begin();
    if let Err(e) = clipboard.write_text(&text) {
        own_seq.cancel();
        return Err(e);
    }
    match clipboard.sequence() {
        Ok(seq) => own_seq.record(seq),
        Err(e) => {
            // The copy itself worked; at worst the watcher bumps the item.
            own_seq.cancel();
            eprintln!("paste: could not read the clipboard sequence: {e}");
        }
    }

    lock(store).touch(item_id, now)?;

    let Some(target) = target else {
        return Ok(PasteOutcome::CopiedOnly(CopiedOnlyReason::NoTarget));
    };
    if !paster.is_trusted() {
        return Ok(PasteOutcome::CopiedOnly(CopiedOnlyReason::NotTrusted));
    }

    hide();
    if !paster.activate(target) {
        return Ok(PasteOutcome::CopiedOnly(
            CopiedOnlyReason::TargetUnavailable,
        ));
    }
    match paster.send_paste() {
        Ok(()) => Ok(PasteOutcome::Pasted),
        Err(e) => {
            eprintln!("paste: {e}");
            Ok(PasteOutcome::CopiedOnly(CopiedOnlyReason::PasteFailed))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::domain::{CoreError, NewCapture};
    use crate::ports::clipboard::fakes::FakeClipboard;
    use crate::ports::frontmost::fakes::FakeFrontmost;
    use crate::ports::paster::fakes::FakePaster;
    use crate::ports::store::fakes::MemoryStore;
    use crate::services::capture::capture_once;

    struct Fixture {
        store: Mutex<MemoryStore>,
        old_id: String,
        clip: FakeClipboard,
        own: OwnWrites,
        hidden: Cell<usize>,
    }

    /// "old text" copied from Notes at t=1, then "new text" from Safari at
    /// t=2, so "old text" is second in the list.
    fn fixture() -> Fixture {
        let mut store = MemoryStore::default();
        let old_id = store
            .record(&NewCapture::text(
                "old text".into(),
                Some("Notes".into()),
                1,
            ))
            .unwrap()
            .id;
        store
            .record(&NewCapture::text(
                "new text".into(),
                Some("Safari".into()),
                2,
            ))
            .unwrap();
        Fixture {
            store: Mutex::new(store),
            old_id,
            clip: FakeClipboard::with_text("new text"),
            own: OwnWrites::default(),
            hidden: Cell::new(0),
        }
    }

    fn textedit() -> TargetApp {
        TargetApp {
            pid: 7,
            name: Some("TextEdit".into()),
            bundle_id: Some("com.apple.TextEdit".into()),
        }
    }

    impl Fixture {
        fn run(&self, target: Option<&TargetApp>, paster: &FakePaster) -> CoreResult<PasteOutcome> {
            paste(
                &self.old_id,
                target,
                &self.store,
                &self.clip,
                paster,
                &self.own,
                10,
                &|| self.hidden.set(self.hidden.get() + 1),
            )
        }

        /// Copied, moved to the top with no new row, and hidden `hidden`
        /// times.
        fn assert_copied_and_bumped(&self, hidden: usize) {
            assert_eq!(*self.clip.writes.borrow(), ["old text"]);
            assert_eq!(self.hidden.get(), hidden);
            let items = lock(&self.store).list_recent(10).unwrap();
            assert_eq!(items.len(), 2, "no new row");
            assert_eq!(items[0].id, self.old_id);
            assert_eq!(items[0].last_copied_at, 10);
            assert_eq!(items[0].source_app.as_deref(), Some("Notes"));
            assert!(self.own.is_own(Some(self.clip.seq.get())));
        }
    }

    #[test]
    fn granted_pastes_into_saved_app() {
        let f = fixture();
        let paster = FakePaster::granted();
        assert_eq!(
            f.run(Some(&textedit()), &paster).unwrap(),
            PasteOutcome::Pasted
        );
        f.assert_copied_and_bumped(1);
        assert_eq!(*paster.activated.borrow(), [7]);
        assert_eq!(paster.pastes.get(), 1);
    }

    #[test]
    fn permission_missing_copies_only_and_keeps_window_open() {
        let f = fixture();
        let paster = FakePaster {
            activates: true,
            ..FakePaster::default()
        };
        assert_eq!(
            f.run(Some(&textedit()), &paster).unwrap(),
            PasteOutcome::CopiedOnly(CopiedOnlyReason::NotTrusted)
        );
        // The UI shows the notice, then returns focus itself.
        f.assert_copied_and_bumped(0);
        assert!(paster.activated.borrow().is_empty(), "no activation yet");
        assert_eq!(paster.pastes.get(), 0);
    }

    #[test]
    fn previous_app_gone_copies_only() {
        // A quit app (or a reused pid) cannot be activated.
        let f = fixture();
        let paster = FakePaster {
            trusted: true,
            activates: false,
            ..FakePaster::default()
        };
        assert_eq!(
            f.run(Some(&textedit()), &paster).unwrap(),
            PasteOutcome::CopiedOnly(CopiedOnlyReason::TargetUnavailable)
        );
        f.assert_copied_and_bumped(1);
        assert_eq!(paster.pastes.get(), 0, "never paste into the wrong app");
    }

    #[test]
    fn activation_timeout_sends_no_keystroke() {
        let f = fixture();
        let paster = FakePaster {
            trusted: true,
            activates: false,
            ..FakePaster::default()
        };
        assert_eq!(
            f.run(Some(&textedit()), &paster).unwrap(),
            PasteOutcome::CopiedOnly(CopiedOnlyReason::TargetUnavailable)
        );
        assert_eq!(*paster.activated.borrow(), [7], "activation was tried");
        assert_eq!(paster.pastes.get(), 0, "no keystroke after a timeout");
        assert_eq!(f.hidden.get(), 1);
    }

    #[test]
    fn no_saved_app_copies_only_and_keeps_window_open() {
        let f = fixture();
        let paster = FakePaster::granted();
        assert_eq!(
            f.run(None, &paster).unwrap(),
            PasteOutcome::CopiedOnly(CopiedOnlyReason::NoTarget)
        );
        f.assert_copied_and_bumped(0);
        assert!(paster.activated.borrow().is_empty());
        assert_eq!(paster.pastes.get(), 0);
    }

    #[test]
    fn failed_keystroke_copies_only() {
        let f = fixture();
        let paster = FakePaster {
            paste_fails: true,
            ..FakePaster::granted()
        };
        assert_eq!(
            f.run(Some(&textedit()), &paster).unwrap(),
            PasteOutcome::CopiedOnly(CopiedOnlyReason::PasteFailed)
        );
        f.assert_copied_and_bumped(1);
    }

    #[test]
    fn vanished_item_is_an_error_and_changes_nothing() {
        let f = fixture();
        let paster = FakePaster::granted();
        let err = paste(
            "missing",
            Some(&textedit()),
            &f.store,
            &f.clip,
            &paster,
            &f.own,
            10,
            &|| f.hidden.set(1),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::NotFound));
        assert!(f.clip.writes.borrow().is_empty());
        assert_eq!(f.hidden.get(), 0, "window stays open");
        assert!(paster.activated.borrow().is_empty());
        assert_eq!(lock(&f.store).list_recent(1).unwrap()[0].last_copied_at, 2);
    }

    #[test]
    fn clipboard_write_failure_is_an_error_and_bumps_nothing() {
        let mut f = fixture();
        f.clip.fail = true;
        let err = f
            .run(Some(&textedit()), &FakePaster::granted())
            .unwrap_err();
        assert!(matches!(err, CoreError::Clipboard(_)));
        assert_eq!(f.hidden.get(), 0);
        let items = lock(&f.store).list_recent(10).unwrap();
        let old = items.iter().find(|i| i.id == f.old_id).unwrap();
        assert_eq!(old.last_copied_at, 1, "not moved to the top");
        assert!(!f.own.is_own(Some(f.clip.seq.get() + 1)), "pending cleared");
    }

    #[test]
    fn own_write_is_not_captured() {
        let f = fixture();
        f.run(Some(&textedit()), &FakePaster::granted()).unwrap();
        // The watcher now sees the change Magpie made, with the pasted text
        // on the clipboard, while another app is frontmost.
        let clip = FakeClipboard::with_text("old text");
        clip.seq.set(f.clip.seq.get());
        let r = capture_once(
            &clip,
            &FakeFrontmost(Some("Terminal".into())),
            &f.store,
            &f.own,
            20,
        )
        .unwrap();
        assert!(r.is_none());
        let items = lock(&f.store).list_recent(10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, f.old_id);
        assert_eq!(items[0].last_copied_at, 10);
        assert_eq!(items[0].source_app.as_deref(), Some("Notes"));

        // A real copy after it is captured as usual.
        clip.external_change();
        let r = capture_once(
            &clip,
            &FakeFrontmost(Some("Terminal".into())),
            &f.store,
            &f.own,
            30,
        )
        .unwrap();
        assert!(r.is_some());
        assert_eq!(
            lock(&f.store).list_recent(1).unwrap()[0]
                .source_app
                .as_deref(),
            Some("Terminal")
        );
    }

    #[test]
    fn own_writes_compare_at_or_just_below() {
        let own = OwnWrites::default();
        assert!(!own.is_own(Some(i64::MIN + 1)), "nothing written yet");
        own.record(Some(500));
        assert!(own.is_own(Some(500)));
        assert!(own.is_own(Some(499)));
        assert!(!own.is_own(Some(501)));
        assert!(!own.is_own(None));
        // A reset or wrap far below is not ours.
        assert!(!own.is_own(Some(1)));
        own.record(None);
        assert!(own.is_own(Some(500)), "unknown sequence keeps the last one");
    }

    #[test]
    fn pending_write_is_own_for_every_sequence() {
        let own = OwnWrites::default();
        own.begin();
        assert!(own.is_own(Some(1)));
        assert!(own.is_own(Some(i64::MAX)));
        assert!(own.is_own(None));
        own.record(Some(10));
        assert!(!own.is_own(Some(11)), "record ends the pending state");
        own.begin();
        own.cancel();
        assert!(!own.is_own(Some(11)), "cancel ends the pending state");
        assert!(own.is_own(Some(10)));
    }
}
