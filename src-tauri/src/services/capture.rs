//! The capture loop (CAP-1): on every clipboard change, read the text and
//! record it. Runs on its own thread under a supervisor (AD-11, AD-18).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::domain::error::CoreResult;
use crate::domain::item::is_blank;
use crate::domain::{CoreError, NewCapture, Recorded};
use crate::ports::{ClipboardReader, ClipboardSequence, ClipboardWatch, Frontmost, Store};
use crate::services::paste::OwnWrites;

/// Locks the store, recovering from poisoning: a panic elsewhere must not
/// stop capture forever, and SQLite keeps its own consistency.
pub fn lock<S: ?Sized>(store: &Mutex<S>) -> MutexGuard<'_, S> {
    store.lock().unwrap_or_else(|e| e.into_inner())
}

/// Handles one clipboard change. Returns what was recorded, or `None` if the
/// change was skipped (Magpie's own write, no text, or whitespace only).
pub fn capture_once<C, F, S>(
    reader: &C,
    frontmost: &F,
    store: &Mutex<S>,
    own: &OwnWrites,
    now: i64,
) -> CoreResult<Option<Recorded>>
where
    C: ClipboardReader + ClipboardSequence + ?Sized,
    F: Frontmost + ?Sized,
    S: Store + ?Sized,
{
    // Magpie's own paste-back writes are not captures (AD-7). Checked before
    // any content is read ...
    if own.is_own(reader.sequence()?) {
        return Ok(None);
    }
    // Formats come first so the privacy filter (AD-6, stage f) can decide
    // before any content is read. An empty clipboard has nothing to keep.
    if reader.formats()?.is_empty() {
        return Ok(None);
    }
    // Stage b keeps plain text only; images, HTML and files come in stage e.
    let Some(text) = reader.text()? else {
        return Ok(None);
    };
    if is_blank(&text) {
        return Ok(None);
    }
    // ... and again after, in case a paste-back write landed while reading.
    if own.is_own(reader.sequence()?) {
        return Ok(None);
    }
    // OS call before taking the lock (AD-11).
    let source_app = frontmost.app_name();
    let capture = NewCapture::text(text, source_app, now);
    lock(store).record(&capture).map(Some)
}

/// Watches the clipboard until the watcher returns, capturing each change and
/// calling `notify` after every recorded one (AD-2).
pub fn watch<W, C, F, S>(
    watcher: &mut W,
    reader: &C,
    frontmost: &F,
    store: &Mutex<S>,
    own: &OwnWrites,
    now: &dyn Fn() -> i64,
    notify: &dyn Fn(),
) -> CoreResult<()>
where
    W: ClipboardWatch + ?Sized,
    C: ClipboardReader + ClipboardSequence + ?Sized,
    F: Frontmost + ?Sized,
    S: Store + ?Sized,
{
    watcher.watch(
        &mut || match capture_once(reader, frontmost, store, own, now()) {
            Ok(Some(_)) => notify(),
            Ok(None) => {}
            // Errors never contain clipboard contents.
            Err(e) => eprintln!("capture: {e}"),
        },
    )
}

/// How one supervised run ended.
#[derive(Debug)]
pub enum RunEnd {
    Returned,
    Failed(CoreError),
    Panicked,
}

/// Runs `run` once, turning a panic into [`RunEnd::Panicked`].
pub fn run_guarded(run: &mut dyn FnMut() -> CoreResult<()>) -> RunEnd {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(())) => RunEnd::Returned,
        Ok(Err(e)) => RunEnd::Failed(e),
        Err(_) => RunEnd::Panicked,
    }
}

pub const MIN_BACKOFF: Duration = Duration::from_secs(1);
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// A run that lasted this long counts as healthy and resets the backoff.
pub const HEALTHY_RUN: Duration = Duration::from_secs(120);

/// The wait before the next restart.
pub fn next_backoff(previous: Duration, ran_for: Duration) -> Duration {
    if ran_for >= HEALTHY_RUN {
        MIN_BACKOFF
    } else {
        (previous * 2).clamp(MIN_BACKOFF, MAX_BACKOFF)
    }
}

/// Runs `run` forever, restarting it with exponential backoff whenever it
/// panics, fails or returns (AD-18). Meant for a dedicated thread.
pub fn supervise(mut run: impl FnMut() -> CoreResult<()>) -> ! {
    let mut backoff = Duration::ZERO;
    loop {
        let started = Instant::now();
        match run_guarded(&mut run) {
            RunEnd::Returned => eprintln!("capture watcher stopped; restarting"),
            RunEnd::Failed(e) => eprintln!("capture watcher failed: {e}; restarting"),
            RunEnd::Panicked => eprintln!("capture watcher panicked; restarting"),
        }
        backoff = next_backoff(backoff, started.elapsed());
        std::thread::sleep(backoff);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::ports::clipboard::fakes::{FakeClipboard, FakeWatch};
    use crate::ports::frontmost::fakes::FakeFrontmost;
    use crate::ports::store::fakes::MemoryStore;

    fn notes() -> FakeFrontmost {
        FakeFrontmost(Some("Notes".into()))
    }

    #[test]
    fn new_text_is_recorded_with_source_app() {
        let store = Mutex::new(MemoryStore::default());
        let r = capture_once(
            &FakeClipboard::with_text("hello"),
            &notes(),
            &store,
            &OwnWrites::default(),
            10,
        )
        .unwrap()
        .unwrap();
        assert!(r.inserted);
        let items = lock(&store).list_recent(10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview, "hello");
        assert_eq!(items[0].source_app.as_deref(), Some("Notes"));
        assert_eq!(items[0].created_at, 10);
        assert_eq!(items[0].last_copied_at, 10);
    }

    #[test]
    fn recopy_bumps_existing_item() {
        let store = Mutex::new(MemoryStore::default());
        let clip = FakeClipboard::with_text("hello");
        let first = capture_once(&clip, &notes(), &store, &OwnWrites::default(), 10)
            .unwrap()
            .unwrap();
        capture_once(
            &FakeClipboard::with_text("other"),
            &notes(),
            &store,
            &OwnWrites::default(),
            20,
        )
        .unwrap();
        let second = capture_once(
            &clip,
            &FakeFrontmost(None),
            &store,
            &OwnWrites::default(),
            30,
        )
        .unwrap()
        .unwrap();
        assert!(!second.inserted);
        assert_eq!(second.id, first.id);
        let items = lock(&store).list_recent(10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, first.id);
        assert_eq!(items[0].last_copied_at, 30);
        // An unknown source on the bump keeps the known one.
        assert_eq!(items[0].source_app.as_deref(), Some("Notes"));
    }

    #[test]
    fn whitespace_only_is_skipped() {
        let store = Mutex::new(MemoryStore::default());
        let r = capture_once(
            &FakeClipboard::with_text("   \n"),
            &notes(),
            &store,
            &OwnWrites::default(),
            1,
        )
        .unwrap();
        assert!(r.is_none());
        assert!(lock(&store).list_recent(10).unwrap().is_empty());
    }

    #[test]
    fn non_text_is_skipped() {
        let store = Mutex::new(MemoryStore::default());
        let r = capture_once(
            &FakeClipboard::image_only(),
            &notes(),
            &store,
            &OwnWrites::default(),
            1,
        )
        .unwrap();
        assert!(r.is_none());
        let empty = capture_once(
            &FakeClipboard::default(),
            &notes(),
            &store,
            &OwnWrites::default(),
            1,
        )
        .unwrap();
        assert!(empty.is_none());
        assert!(lock(&store).list_recent(10).unwrap().is_empty());
    }

    #[test]
    fn clipboard_errors_propagate() {
        let store = Mutex::new(MemoryStore::default());
        let clip = FakeClipboard {
            fail: true,
            ..FakeClipboard::with_text("x")
        };
        assert!(capture_once(&clip, &notes(), &store, &OwnWrites::default(), 1).is_err());
    }

    #[test]
    fn watch_notifies_once_per_recorded_change() {
        let store = Mutex::new(MemoryStore::default());
        let notified = Cell::new(0);
        watch(
            &mut FakeWatch { changes: 3 },
            &FakeClipboard::with_text("hello"),
            &notes(),
            &store,
            &OwnWrites::default(),
            &|| 5,
            &|| notified.set(notified.get() + 1),
        )
        .unwrap();
        // Three changes of the same text: one row, bumped, each one notifies.
        assert_eq!(notified.get(), 3);
        assert_eq!(lock(&store).list_recent(10).unwrap().len(), 1);

        let skipped = Cell::new(0);
        watch(
            &mut FakeWatch { changes: 2 },
            &FakeClipboard::with_text(" "),
            &notes(),
            &store,
            &OwnWrites::default(),
            &|| 6,
            &|| skipped.set(skipped.get() + 1),
        )
        .unwrap();
        assert_eq!(skipped.get(), 0);
    }

    #[test]
    fn own_writes_at_or_below_are_skipped() {
        let store = Mutex::new(MemoryStore::default());
        let own = OwnWrites::default();
        own.record(Some(3));
        let clip = FakeClipboard::with_text("pasted");
        for seq in [2, 3] {
            clip.seq.set(seq);
            assert!(
                capture_once(&clip, &notes(), &store, &own, 1)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(lock(&store).list_recent(10).unwrap().is_empty());
        clip.seq.set(4);
        assert!(
            capture_once(&clip, &notes(), &store, &own, 2)
                .unwrap()
                .is_some()
        );
    }

    /// A paste-back write that lands between the first sequence check and
    /// reading the text is caught by the second check.
    #[test]
    fn own_write_during_read_is_skipped() {
        struct Racy {
            inner: FakeClipboard,
            own: std::sync::Arc<OwnWrites>,
        }
        impl ClipboardReader for Racy {
            fn formats(&self) -> CoreResult<Vec<String>> {
                self.inner.formats()
            }
            fn text(&self) -> CoreResult<Option<String>> {
                // Magpie writes now.
                self.inner.external_change();
                self.own.record(self.inner.sequence()?);
                self.inner.text()
            }
        }
        impl ClipboardSequence for Racy {
            fn sequence(&self) -> CoreResult<Option<i64>> {
                self.inner.sequence()
            }
        }
        let own = std::sync::Arc::new(OwnWrites::default());
        let racy = Racy {
            inner: FakeClipboard::with_text("pasted"),
            own: own.clone(),
        };
        let store = Mutex::new(MemoryStore::default());
        assert!(
            capture_once(&racy, &notes(), &store, &own, 1)
                .unwrap()
                .is_none()
        );
        assert!(lock(&store).list_recent(10).unwrap().is_empty());
    }

    #[test]
    fn panics_are_contained() {
        let mut calls = 0;
        let end = run_guarded(&mut || {
            calls += 1;
            panic!("adapter blew up");
        });
        assert!(matches!(end, RunEnd::Panicked));
        assert_eq!(calls, 1);
        assert!(matches!(
            run_guarded(&mut || Err(CoreError::Clipboard("x".into()))),
            RunEnd::Failed(_)
        ));
        assert!(matches!(run_guarded(&mut || Ok(())), RunEnd::Returned));
    }

    #[test]
    fn store_survives_a_poisoned_lock() {
        let store = std::sync::Arc::new(Mutex::new(MemoryStore::default()));
        let s2 = store.clone();
        let _ = std::thread::spawn(move || {
            let _g = s2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert!(store.is_poisoned());
        let r = capture_once(
            &FakeClipboard::with_text("ok"),
            &notes(),
            &*store,
            &OwnWrites::default(),
            1,
        )
        .unwrap();
        assert!(r.is_some());
    }

    #[test]
    fn backoff_doubles_caps_and_resets() {
        let quick = Duration::from_millis(10);
        assert_eq!(next_backoff(Duration::ZERO, quick), MIN_BACKOFF);
        assert_eq!(next_backoff(MIN_BACKOFF, quick), Duration::from_secs(2));
        assert_eq!(next_backoff(MAX_BACKOFF, quick), MAX_BACKOFF);
        assert_eq!(next_backoff(MAX_BACKOFF, HEALTHY_RUN), MIN_BACKOFF);
    }
}
