//! Tauri commands and their DTOs (AD-9).

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::error::{AppError, AppResult};
use super::state::{AppState, SharedStore};
use super::{EVENT_HISTORY_CHANGED, window};
use crate::adapters::SystemPaster;
use crate::adapters::clipboard::SystemClipboard;
use crate::domain::ItemSummary;
use crate::ports::{ClipboardWriter, Paster, Store};
use crate::services::capture::lock;
use crate::services::now_ms;
use crate::services::paste::{self, CopiedOnlyReason, PasteOutcome};

/// Upper bound on `search`'s `limit`.
pub const MAX_LIST_LIMIT: u32 = 1000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummaryDto {
    pub id: String,
    pub kind: &'static str,
    pub preview: String,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub last_copied_at: i64,
    pub pinned: bool,
}

impl From<ItemSummary> for ItemSummaryDto {
    fn from(i: ItemSummary) -> Self {
        Self {
            id: i.id,
            kind: i.kind.as_str(),
            preview: i.preview,
            source_app: i.source_app,
            created_at: i.created_at,
            last_copied_at: i.last_copied_at,
            pinned: i.pinned,
        }
    }
}

fn store(state: &AppState) -> AppResult<&SharedStore> {
    state
        .store
        .as_ref()
        .map_err(|e| AppError::StoreUnavailable(e.clone()))
}

/// Items matching `query` as the user typed it (each token a prefix, all
/// required, AD-8), newest first; a missing or blank query lists by recency.
/// `pinned_only` serves the Pinned view. Async so typing never blocks the
/// main thread while the watcher holds the store lock.
#[tauri::command(async)]
pub fn search(
    state: State<'_, AppState>,
    query: Option<String>,
    pinned_only: bool,
    limit: u32,
) -> AppResult<Vec<ItemSummaryDto>> {
    search_impl(&state, query.as_deref(), pinned_only, limit)
}

fn search_impl(
    state: &AppState,
    query: Option<&str>,
    pinned_only: bool,
    limit: u32,
) -> AppResult<Vec<ItemSummaryDto>> {
    let store = store(state)?;
    let limit = limit.clamp(1, MAX_LIST_LIMIT) as usize;
    let items = lock(store).search(query, pinned_only, limit)?;
    Ok(items.into_iter().map(Into::into).collect())
}

/// Pins or unpins item `id` (CAP-6), then tells the UI to refetch (AD-2).
/// An unknown id is a readable error.
#[tauri::command(async)]
pub fn set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> AppResult<()> {
    let result = set_pinned_impl(&state, &id, pinned);
    // Emitted on failure too: a missing item means the UI's list is stale.
    if let Err(e) = app.emit(EVENT_HISTORY_CHANGED, ()) {
        eprintln!("could not emit {EVENT_HISTORY_CHANGED}: {e}");
    }
    result
}

fn set_pinned_impl(state: &AppState, id: &str, pinned: bool) -> AppResult<()> {
    let store = store(state)?;
    lock(store).set_pinned(id, pinned)?;
    Ok(())
}

/// Why history is unavailable, or `None` if the database opened fine.
#[tauri::command(async)]
pub fn startup_error(state: State<'_, AppState>) -> Option<String> {
    startup_error_impl(&state)
}

fn startup_error_impl(state: &AppState) -> Option<String> {
    state.store.as_ref().err().cloned()
}

/// What happened on paste-back (AD-12). Serialised as
/// `{ "outcome": "pasted" }` or `{ "outcome": "copiedOnly", "reason": … }`.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum PasteOutcomeDto {
    Pasted,
    CopiedOnly { reason: &'static str },
}

impl From<PasteOutcome> for PasteOutcomeDto {
    fn from(o: PasteOutcome) -> Self {
        match o {
            PasteOutcome::Pasted => Self::Pasted,
            PasteOutcome::CopiedOnly(r) => Self::CopiedOnly {
                reason: match r {
                    CopiedOnlyReason::NotTrusted => "notTrusted",
                    CopiedOnlyReason::NoTarget => "noTarget",
                    CopiedOnlyReason::TargetUnavailable => "targetUnavailable",
                    CopiedOnlyReason::PasteFailed => "pasteFailed",
                },
            },
        }
    }
}

/// Copies item `id` and, when allowed, hides the window and pastes it into
/// the app that was frontmost when the window opened (CAP-5). Without the
/// permission or a saved app the window stays open (`notTrusted`,
/// `noTarget`) so the UI can say "copied"; it then calls
/// [`return_to_previous_app`]. A missing item is an error and leaves the
/// window open. Async: it waits for the target app to come to the front,
/// which must not block the main thread.
#[tauri::command(async)]
pub fn paste_item(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<PasteOutcomeDto> {
    let clipboard = SystemClipboard::new()?;
    let result = paste_item_impl(&state, &id, &clipboard, &SystemPaster, now_ms(), &|| {
        window::hide(&app)
    });
    // The item moved to the top, or vanished: either way the list changed
    // (AD-2).
    if let Err(e) = app.emit(EVENT_HISTORY_CHANGED, ()) {
        eprintln!("could not emit {EVENT_HISTORY_CHANGED}: {e}");
    }
    result
}

fn paste_item_impl<C, P>(
    state: &AppState,
    id: &str,
    clipboard: &C,
    paster: &P,
    now: i64,
    hide: &dyn Fn(),
) -> AppResult<PasteOutcomeDto>
where
    C: ClipboardWriter + ?Sized,
    P: Paster + ?Sized,
{
    let store = store(state)?;
    let target = lock(&state.target).clone();
    let outcome = paste::paste(
        id,
        target.as_ref(),
        &**store,
        clipboard,
        paster,
        &state.own_writes,
        now,
        hide,
    )?;
    Ok(outcome.into())
}

/// After a copy-only paste: hides the window and gives focus back to the app
/// that was frontmost when it opened, if any.
#[tauri::command(async)]
pub fn return_to_previous_app(app: AppHandle, state: State<'_, AppState>) {
    return_to_previous_app_impl(&state, &SystemPaster, &|| window::hide(&app));
}

fn return_to_previous_app_impl<P: Paster + ?Sized>(state: &AppState, paster: &P, hide: &dyn Fn()) {
    // Read first: hiding clears the saved target.
    let target = lock(&state.target).clone();
    hide();
    if let Some(target) = target {
        // Best effort: if the app is gone there is nothing to return to.
        paster.activate(&target);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityStatusDto {
    /// This platform has a paste permission (macOS Accessibility). When
    /// false the UI shows no explainer and names Ctrl+V instead of ⌘V.
    pub applies: bool,
    /// Magpie may send the synthetic paste keystroke.
    pub trusted: bool,
    /// The user has already answered the first-run explainer.
    pub asked: bool,
}

#[tauri::command(async)]
pub fn accessibility_status(state: State<'_, AppState>) -> AccessibilityStatusDto {
    accessibility_status_impl(&state, &SystemPaster)
}

fn accessibility_status_impl<P: Paster + ?Sized>(
    state: &AppState,
    paster: &P,
) -> AccessibilityStatusDto {
    AccessibilityStatusDto {
        applies: paster.trust_applies(),
        trusted: paster.is_trusted(),
        asked: state
            .permission_asked_marker
            .as_deref()
            .is_some_and(std::path::Path::exists),
    }
}

/// Remembers that the explainer was answered, so it is not shown again.
fn mark_asked(state: &AppState) -> AppResult<()> {
    let Some(marker) = state.permission_asked_marker.as_deref() else {
        return Err(AppError::PermissionChoice(
            "no data directory is available".into(),
        ));
    };
    if let Some(dir) = marker.parent() {
        std::fs::create_dir_all(dir).map_err(|e| AppError::PermissionChoice(e.to_string()))?;
    }
    std::fs::write(marker, b"").map_err(|e| AppError::PermissionChoice(e.to_string()))
}

fn mark_asked_or_log(state: &AppState) {
    if let Err(e) = mark_asked(state) {
        eprintln!("{e}");
    }
}

/// Shows the system Accessibility prompt (the first time only; afterwards
/// the UI opens the settings pane instead). Returns whether Magpie is
/// trusted right now (granting happens later, in System Settings).
#[tauri::command(async)]
pub fn request_accessibility(state: State<'_, AppState>) -> bool {
    let trusted = SystemPaster.request_trust();
    mark_asked_or_log(&state);
    trusted
}

/// Opens System Settings at Privacy & Security > Accessibility.
#[tauri::command(async)]
pub fn open_accessibility_settings(state: State<'_, AppState>) -> AppResult<()> {
    open_accessibility_settings_impl(&state, &SystemPaster)
}

fn open_accessibility_settings_impl<P: Paster + ?Sized>(
    state: &AppState,
    paster: &P,
) -> AppResult<()> {
    paster.open_trust_settings()?;
    // Only once the pane actually opened.
    mark_asked_or_log(state);
    Ok(())
}

/// "Not now" on the first-run explainer.
#[tauri::command(async)]
pub fn dismiss_accessibility_prompt(state: State<'_, AppState>) -> AppResult<()> {
    mark_asked(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::adapters::sqlite::SqliteStore;
    use crate::domain::{ItemKind, NewCapture, TargetApp};
    use crate::ports::clipboard::fakes::FakeClipboard;
    use crate::ports::paster::fakes::FakePaster;

    fn state_with(
        store: Result<SharedStore, String>,
        marker: Option<std::path::PathBuf>,
    ) -> AppState {
        AppState::new(store, Arc::new(Default::default()), marker)
    }

    fn sqlite_state(dir: &tempfile::TempDir) -> (AppState, String) {
        let mut store = SqliteStore::open(&dir.path().join("magpie.db")).unwrap();
        let id = store
            .record(&NewCapture::text("hello".into(), Some("Notes".into()), 1))
            .unwrap()
            .id;
        store
            .record(&NewCapture::text("newer".into(), None, 2))
            .unwrap();
        let state = state_with(
            Ok(Arc::new(Mutex::new(store))),
            Some(dir.path().join("accessibility-asked")),
        );
        (state, id)
    }

    #[test]
    fn unavailable_store_reports_its_error() {
        let state = state_with(Err("disk is read-only".into()), None);
        assert_eq!(
            startup_error_impl(&state).as_deref(),
            Some("disk is read-only")
        );
        let err = search_impl(&state, None, false, 10)
            .unwrap_err()
            .to_string();
        assert!(err.contains("disk is read-only"), "{err}");
    }

    #[test]
    fn available_store_lists_items() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqliteStore::open(&dir.path().join("magpie.db")).unwrap();
        store
            .record(&NewCapture::text("hello".into(), None, 1))
            .unwrap();
        let state = state_with(Ok(Arc::new(Mutex::new(store))), None);
        assert_eq!(startup_error_impl(&state), None);
        let items = search_impl(&state, None, false, 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview, "hello");
    }

    #[test]
    fn search_command_filters_and_pins() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id) = sqlite_state(&dir);
        let found = search_impl(&state, Some("hel"), false, 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, id);
        assert_eq!(search_impl(&state, Some(" "), false, 10).unwrap().len(), 2);
        assert!(search_impl(&state, None, true, 10).unwrap().is_empty());

        set_pinned_impl(&state, &id, true).unwrap();
        let pinned = search_impl(&state, None, true, 10).unwrap();
        assert_eq!(pinned.len(), 1);
        assert!(pinned[0].pinned);
        assert!(
            search_impl(&state, Some("newer"), true, 10)
                .unwrap()
                .is_empty()
        );

        let err = set_pinned_impl(&state, "gone", true).unwrap_err();
        assert_eq!(err.to_string(), "This item is no longer in history");
        let err = search_impl(&state_with(Err("disk".into()), None), None, false, 10)
            .unwrap_err()
            .to_string();
        assert!(err.contains("disk"), "{err}");
    }

    #[test]
    fn dto_is_camel_case_with_lowercase_kind() {
        let dto: ItemSummaryDto = ItemSummary {
            id: "x".into(),
            kind: ItemKind::Text,
            preview: "hi".into(),
            source_app: Some("Notes".into()),
            created_at: 1,
            last_copied_at: 2,
            pinned: false,
        }
        .into();
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["kind"], "text");
        assert_eq!(json["sourceApp"], "Notes");
        assert_eq!(json["lastCopiedAt"], 2);
        assert_eq!(json["createdAt"], 1);
    }

    #[test]
    fn paste_item_uses_saved_target_and_moves_item_to_top() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id) = sqlite_state(&dir);
        *lock(&state.target) = Some(TargetApp {
            pid: 7,
            name: Some("TextEdit".into()),
            bundle_id: Some("com.apple.TextEdit".into()),
        });
        let clip = FakeClipboard::default();
        let paster = FakePaster::granted();
        let out = paste_item_impl(&state, &id, &clip, &paster, 50, &|| {}).unwrap();
        assert_eq!(out, PasteOutcomeDto::Pasted);
        assert_eq!(*paster.activated.borrow(), [7]);
        assert_eq!(*clip.writes.borrow(), ["hello"]);
        let items = search_impl(&state, None, false, 10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, id);
        assert_eq!(items[0].source_app.as_deref(), Some("Notes"));
        assert!(state.own_writes.is_own(Some(1)));
    }

    #[test]
    fn not_trusted_with_saved_target_copies_and_keeps_window_open() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id) = sqlite_state(&dir);
        *lock(&state.target) = Some(TargetApp {
            pid: 7,
            name: Some("TextEdit".into()),
            bundle_id: Some("com.apple.TextEdit".into()),
        });
        let clip = FakeClipboard::default();
        let paster = FakePaster {
            activates: true,
            ..FakePaster::default()
        };
        let hidden = std::cell::Cell::new(0);
        let out = paste_item_impl(&state, &id, &clip, &paster, 50, &|| {
            hidden.set(hidden.get() + 1)
        })
        .unwrap();
        assert_eq!(
            out,
            PasteOutcomeDto::CopiedOnly {
                reason: "notTrusted"
            }
        );
        assert_eq!(*clip.writes.borrow(), ["hello"]);
        assert_eq!(hidden.get(), 0, "window stays open for the notice");
        assert!(paster.activated.borrow().is_empty());
        assert_eq!(search_impl(&state, None, false, 1).unwrap()[0].id, id);

        // Then the UI returns focus: hide, and reactivate the saved app.
        return_to_previous_app_impl(&state, &paster, &|| hidden.set(hidden.get() + 1));
        assert_eq!(hidden.get(), 1);
        assert_eq!(*paster.activated.borrow(), [7]);
        assert_eq!(paster.pastes.get(), 0);
    }

    #[test]
    fn return_without_saved_target_only_hides() {
        let state = state_with(Err("x".into()), None);
        let paster = FakePaster::granted();
        let hidden = std::cell::Cell::new(false);
        return_to_previous_app_impl(&state, &paster, &|| hidden.set(true));
        assert!(hidden.get());
        assert!(paster.activated.borrow().is_empty());
    }

    #[test]
    fn paste_item_without_permission_is_copied_only() {
        let dir = tempfile::tempdir().unwrap();
        let (state, id) = sqlite_state(&dir);
        let out = paste_item_impl(
            &state,
            &id,
            &FakeClipboard::default(),
            &FakePaster::default(),
            50,
            &|| {},
        )
        .unwrap();
        // No target was saved either; the missing target is reported first.
        assert_eq!(out, PasteOutcomeDto::CopiedOnly { reason: "noTarget" });
        let json = serde_json::to_value(PasteOutcomeDto::from(PasteOutcome::CopiedOnly(
            CopiedOnlyReason::NotTrusted,
        )))
        .unwrap();
        assert_eq!(json["outcome"], "copiedOnly");
        assert_eq!(json["reason"], "notTrusted");
        assert_eq!(
            serde_json::to_value(PasteOutcomeDto::Pasted).unwrap()["outcome"],
            "pasted"
        );
    }

    #[test]
    fn paste_of_vanished_item_is_a_readable_error() {
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = sqlite_state(&dir);
        let hidden = std::cell::Cell::new(false);
        let err = paste_item_impl(
            &state,
            "gone",
            &FakeClipboard::default(),
            &FakePaster::granted(),
            50,
            &|| hidden.set(true),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "This item is no longer in history");
        assert!(!hidden.get());
    }

    #[test]
    fn explainer_is_asked_once() {
        let dir = tempfile::tempdir().unwrap();
        let (state, _) = sqlite_state(&dir);
        let paster = FakePaster {
            applies: true,
            ..FakePaster::default()
        };
        let s = accessibility_status_impl(&state, &paster);
        assert!(s.applies && !s.trusted && !s.asked);

        // A pane that fails to open does not count as answered.
        let failing = FakePaster {
            settings_fail: true,
            ..FakePaster::default()
        };
        let err = open_accessibility_settings_impl(&state, &failing)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Paste-back error"), "{err}");
        assert!(!accessibility_status_impl(&state, &paster).asked);

        open_accessibility_settings_impl(&state, &paster).unwrap();
        assert!(accessibility_status_impl(&state, &paster).asked);

        let json = serde_json::to_value(accessibility_status_impl(&state, &FakePaster::granted()))
            .unwrap();
        assert_eq!(json["trusted"], true);
        assert_eq!(json["applies"], false);
        assert_eq!(json["asked"], true);
    }

    #[test]
    fn dismiss_without_data_dir_is_a_readable_error() {
        let state = state_with(Err("x".into()), None);
        let err = mark_asked(&state).unwrap_err().to_string();
        assert!(err.contains("Accessibility"), "{err}");
    }
}
