//! SQLite history store (AD-16): `seq` primary key, stable `id`, FTS5 index
//! kept in sync by triggers, WAL journal.

mod migrations;

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::error::CoreResult;
use crate::domain::item::preview;
use crate::domain::{CoreError, ItemKind, ItemSummary, NewCapture, Recorded};
use crate::ports::Store;

/// Characters fetched from SQLite per row (after trimming leading space, tab,
/// LF and CR) before the domain caps the preview, so a 1 MB item is never
/// loaded for a list.
const PREVIEW_FETCH_CHARS: i64 = 2000;

pub(crate) fn db_err(e: rusqlite::Error) -> CoreError {
    CoreError::Storage(e.to_string())
}

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Opens (creating if needed) the database at `path` and migrates it.
    pub fn open(path: &Path) -> CoreResult<Self> {
        let mut conn = Connection::open(path)
            .map_err(|e| CoreError::Storage(format!("could not open {}: {e}", path.display())))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_err)?;
        // journal_mode returns the resulting mode as a row.
        let _mode: String = conn
            .query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(db_err)?;
        migrations::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    #[cfg(test)]
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl Store for SqliteStore {
    fn record(&mut self, c: &NewCapture) -> CoreResult<Recorded> {
        // Never INSERT OR REPLACE (AD-16): that would delete the row and give
        // it a new `seq`. On a dedupe hit only the recency and source change;
        // an unknown source keeps the known one.
        let id: String = self
            .conn
            .query_row(
                "INSERT INTO items
                    (id, kind, content_hash, plain_text, source_app, created_at, last_copied_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT (content_hash) DO UPDATE SET
                    last_copied_at = excluded.last_copied_at,
                    source_app = COALESCE(excluded.source_app, items.source_app)
                 RETURNING id",
                params![
                    c.id,
                    c.kind.as_str(),
                    c.content_hash,
                    c.plain_text,
                    c.source_app,
                    c.at
                ],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        let inserted = id == c.id;
        Ok(Recorded { id, inserted })
    }

    fn list_recent(&self, limit: usize) -> CoreResult<Vec<ItemSummary>> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT id, kind, substr(ltrim(plain_text, ' ' || char(9, 10, 13)), 1, ?1), source_app,
                        created_at, last_copied_at, pinned
                 FROM items
                 ORDER BY last_copied_at DESC, seq DESC
                 LIMIT ?2",
            )
            .map_err(db_err)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![PREVIEW_FETCH_CHARS, limit], |r| {
                let kind: String = r.get(1)?;
                let text: Option<String> = r.get(2)?;
                Ok((
                    kind,
                    ItemSummary {
                        id: r.get(0)?,
                        kind: ItemKind::Text,
                        preview: text.as_deref().map(preview).unwrap_or_default(),
                        source_app: r.get(3)?,
                        created_at: r.get(4)?,
                        last_copied_at: r.get(5)?,
                        pinned: r.get::<_, i64>(6)? != 0,
                    },
                ))
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (kind, mut item) = row.map_err(db_err)?;
            item.kind = ItemKind::parse(&kind)
                .ok_or_else(|| CoreError::Storage(format!("unknown item kind {kind:?}")))?;
            out.push(item);
        }
        Ok(out)
    }

    fn get_text(&self, id: &str) -> CoreResult<String> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT plain_text FROM items WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_err)?
            .ok_or(CoreError::NotFound)?;
        // Every stage-b/c item is text; non-text kinds arrive in stage e.
        text.ok_or_else(|| CoreError::Storage("this item has no text".into()))
    }

    fn touch(&mut self, id: &str, at: i64) -> CoreResult<()> {
        // Only the recency changes; `source_app`, `id` and `pinned` stay
        // (AD-7). An UPDATE keeps `seq`, so the FTS row is untouched.
        let changed = self
            .conn
            .execute(
                "UPDATE items SET last_copied_at = ?2 WHERE id = ?1",
                params![id, at],
            )
            .map_err(db_err)?;
        if changed == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(&dir.path().join("magpie.db")).unwrap();
        (dir, store)
    }

    fn count(store: &SqliteStore, sql: &str) -> i64 {
        store.conn().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn opens_with_wal_and_schema_v1() {
        let (_d, s) = temp_store();
        let mode: String = s
            .conn()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        assert_eq!(count(&s, "PRAGMA user_version"), migrations::LATEST);
    }

    #[test]
    fn reopening_keeps_history_and_does_not_remigrate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("magpie.db");
        {
            let mut s = SqliteStore::open(&path).unwrap();
            s.record(&NewCapture::text("persisted".into(), None, 1))
                .unwrap();
        }
        let s = SqliteStore::open(&path).unwrap();
        let items = s.list_recent(10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview, "persisted");
    }

    #[test]
    fn newer_schema_is_refused_with_readable_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("magpie.db");
        {
            let c = Connection::open(&path).unwrap();
            c.pragma_update(None, "user_version", 99).unwrap();
        }
        let err = SqliteStore::open(&path).err().unwrap().to_string();
        assert!(err.contains("newer version"), "{err}");
    }

    #[test]
    fn unopenable_path_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("sub").join("magpie.db");
        assert!(matches!(
            SqliteStore::open(&path),
            Err(CoreError::Storage(_))
        ));
    }

    #[test]
    fn insert_then_recopy_bumps_same_row() {
        let (_d, mut s) = temp_store();
        let first = s
            .record(&NewCapture::text("hello".into(), Some("Notes".into()), 100))
            .unwrap();
        assert!(first.inserted);
        s.record(&NewCapture::text("other".into(), None, 200))
            .unwrap();

        let again = s
            .record(&NewCapture::text(
                "hello".into(),
                Some("Safari".into()),
                300,
            ))
            .unwrap();
        assert!(!again.inserted);
        assert_eq!(again.id, first.id);
        assert_eq!(count(&s, "SELECT count(*) FROM items"), 2);

        let items = s.list_recent(10).unwrap();
        assert_eq!(items[0].id, first.id);
        assert_eq!(items[0].last_copied_at, 300);
        assert_eq!(items[0].created_at, 100);
        assert_eq!(items[0].source_app.as_deref(), Some("Safari"));
        assert_eq!(items[1].preview, "other");
    }

    #[test]
    fn bump_with_unknown_source_keeps_known_source() {
        let (_d, mut s) = temp_store();
        s.record(&NewCapture::text("hello".into(), Some("Notes".into()), 1))
            .unwrap();
        s.record(&NewCapture::text("hello".into(), None, 2))
            .unwrap();
        let items = s.list_recent(1).unwrap();
        assert_eq!(items[0].source_app.as_deref(), Some("Notes"));
        assert_eq!(items[0].last_copied_at, 2);
    }

    #[test]
    fn preview_skips_long_leading_whitespace() {
        let (_d, mut s) = temp_store();
        let text = format!("{}hello", " \t\r\n".repeat(1000));
        s.record(&NewCapture::text(text, None, 1)).unwrap();
        assert_eq!(s.list_recent(1).unwrap()[0].preview, "hello");
    }

    #[test]
    fn equal_timestamps_list_newest_insert_first() {
        let (_d, mut s) = temp_store();
        s.record(&NewCapture::text("first".into(), None, 5))
            .unwrap();
        s.record(&NewCapture::text("second".into(), None, 5))
            .unwrap();
        let previews: Vec<_> = s
            .list_recent(10)
            .unwrap()
            .into_iter()
            .map(|i| i.preview)
            .collect();
        assert_eq!(previews, ["second", "first"]);
    }

    #[test]
    fn negative_schema_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("magpie.db");
        {
            let c = Connection::open(&path).unwrap();
            c.pragma_update(None, "user_version", -1).unwrap();
        }
        let err = SqliteStore::open(&path).err().unwrap().to_string();
        assert!(err.contains("invalid schema version"), "{err}");
    }

    #[test]
    fn list_is_newest_first_and_limited() {
        let (_d, mut s) = temp_store();
        for (i, t) in ["a", "b", "c"].iter().enumerate() {
            s.record(&NewCapture::text((*t).into(), None, i as i64))
                .unwrap();
        }
        let items = s.list_recent(2).unwrap();
        let previews: Vec<_> = items.iter().map(|i| i.preview.as_str()).collect();
        assert_eq!(previews, ["c", "b"]);
        assert_eq!(items[0].kind, ItemKind::Text);
        assert!(!items[0].pinned);
    }

    #[test]
    fn long_text_is_stored_in_full_and_previewed_short() {
        let (_d, mut s) = temp_store();
        let big = "x".repeat(1024 * 1024);
        let r = s.record(&NewCapture::text(big.clone(), None, 1)).unwrap();
        assert_eq!(s.get_text(&r.id).unwrap(), big);
        let items = s.list_recent(1).unwrap();
        assert_eq!(items[0].preview.chars().count(), 200);
    }

    #[test]
    fn get_text_returns_full_text_or_not_found() {
        let (_d, mut s) = temp_store();
        let r = s
            .record(&NewCapture::text("  hello\n".into(), None, 1))
            .unwrap();
        assert_eq!(s.get_text(&r.id).unwrap(), "  hello\n");
        assert!(matches!(s.get_text("missing"), Err(CoreError::NotFound)));
    }

    #[test]
    fn touch_moves_item_to_top_and_keeps_everything_else() {
        let (_d, mut s) = temp_store();
        let old = s
            .record(&NewCapture::text("old".into(), Some("Notes".into()), 1))
            .unwrap();
        s.record(&NewCapture::text("new".into(), Some("Safari".into()), 2))
            .unwrap();
        let seq_before = count(
            &s,
            "SELECT seq FROM items WHERE content_hash IS NOT NULL ORDER BY created_at LIMIT 1",
        );
        s.touch(&old.id, 3).unwrap();

        let items = s.list_recent(10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, old.id);
        assert_eq!(items[0].last_copied_at, 3);
        assert_eq!(items[0].created_at, 1);
        assert_eq!(items[0].source_app.as_deref(), Some("Notes"));
        assert_eq!(
            count(
                &s,
                "SELECT seq FROM items WHERE content_hash IS NOT NULL ORDER BY created_at LIMIT 1"
            ),
            seq_before
        );
        assert!(matches!(s.touch("missing", 4), Err(CoreError::NotFound)));
    }

    #[test]
    fn fts_follows_inserts_and_is_not_duplicated_by_bumps() {
        let (_d, mut s) = temp_store();
        s.record(&NewCapture::text("héllo world".into(), None, 1))
            .unwrap();
        s.record(&NewCapture::text("héllo world".into(), None, 2))
            .unwrap();
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM items_fts WHERE items_fts MATCH '\"hello\"*'"
            ),
            1
        );
        s.conn().execute("DELETE FROM items", []).unwrap();
        assert_eq!(
            count(
                &s,
                "SELECT count(*) FROM items_fts WHERE items_fts MATCH 'world'"
            ),
            0
        );
    }
}
