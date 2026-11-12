//! Tauri commands and their DTOs (AD-9).

use serde::Serialize;
use tauri::State;

use super::error::{AppError, AppResult};
use super::state::AppState;
use crate::domain::ItemSummary;
use crate::ports::Store;
use crate::services::capture::lock;

/// Upper bound on `list_items`' `limit`.
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

/// The most recent items, newest first. Async so it runs off the main thread
/// and never blocks the UI while the watcher holds the store lock.
#[tauri::command(async)]
pub fn list_items(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<ItemSummaryDto>> {
    list_items_impl(&state, limit)
}

fn list_items_impl(state: &AppState, limit: u32) -> AppResult<Vec<ItemSummaryDto>> {
    let store = state
        .store
        .as_ref()
        .map_err(|e| AppError::StoreUnavailable(e.clone()))?;
    let limit = limit.clamp(1, MAX_LIST_LIMIT) as usize;
    let items = lock(store).list_recent(limit)?;
    Ok(items.into_iter().map(Into::into).collect())
}

/// Why history is unavailable, or `None` if the database opened fine.
#[tauri::command(async)]
pub fn startup_error(state: State<'_, AppState>) -> Option<String> {
    startup_error_impl(&state)
}

fn startup_error_impl(state: &AppState) -> Option<String> {
    state.store.as_ref().err().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::adapters::sqlite::SqliteStore;
    use crate::domain::{ItemKind, NewCapture};

    #[test]
    fn unavailable_store_reports_its_error() {
        let state = AppState {
            store: Err("disk is read-only".into()),
        };
        assert_eq!(
            startup_error_impl(&state).as_deref(),
            Some("disk is read-only")
        );
        let err = list_items_impl(&state, 10).unwrap_err().to_string();
        assert!(err.contains("disk is read-only"), "{err}");
    }

    #[test]
    fn available_store_lists_items() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqliteStore::open(&dir.path().join("magpie.db")).unwrap();
        store
            .record(&NewCapture::text("hello".into(), None, 1))
            .unwrap();
        let state = AppState {
            store: Ok(Arc::new(Mutex::new(store))),
        };
        assert_eq!(startup_error_impl(&state), None);
        let items = list_items_impl(&state, 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview, "hello");
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
}
