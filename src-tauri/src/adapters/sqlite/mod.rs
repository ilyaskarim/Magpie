//! SQLite history store (AD-16): `seq` primary key, stable `id`, FTS5 index
//! kept in sync by triggers, WAL journal.

mod migrations;

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::error::CoreResult;
use crate::domain::item::preview;
use crate::domain::search::fts_query;
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

    fn search(
        &self,
        query: Option<&str>,
        pinned_only: bool,
        limit: usize,
    ) -> CoreResult<Vec<ItemSummary>> {
        // The user's text only ever reaches SQLite as a bound parameter, and
        // `fts_query` quotes every token, so it cannot inject FTS5 syntax or
        // SQL (AD-8). Ordering is by recency, not relevance.
        let fts = query.and_then(fts_query);
        let filter = if fts.is_some() {
            "WHERE seq IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?3)
               AND (?4 = 0 OR pinned = 1)"
        } else {
            "WHERE (?3 IS NULL) AND (?4 = 0 OR pinned = 1)"
        };
        let sql = format!(
            "SELECT id, kind, substr(ltrim(plain_text, ' ' || char(9, 10, 13)), 1, ?1), source_app,
                    created_at, last_copied_at, pinned
             FROM items
             {filter}
             ORDER BY last_copied_at DESC, seq DESC
             LIMIT ?2"
        );
        let mut stmt = self.conn.prepare_cached(&sql).map_err(db_err)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![PREVIEW_FETCH_CHARS, limit, fts, pinned_only], |r| {
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

    fn set_pinned(&mut self, id: &str, pinned: bool) -> CoreResult<()> {
        // An UPDATE keeps `seq` and does not touch `plain_text`, so the FTS
        // row is untouched and the recency does not change.
        let changed = self
            .conn
            .execute(
                "UPDATE items SET pinned = ?2 WHERE id = ?1",
                params![id, pinned],
            )
            .map_err(db_err)?;
        if changed == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
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

    fn previews(items: Vec<ItemSummary>) -> Vec<String> {
        items.into_iter().map(|i| i.preview).collect()
    }

    fn seeded(texts: &[&str]) -> (tempfile::TempDir, SqliteStore, Vec<String>) {
        let (d, mut s) = temp_store();
        let ids = texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                s.record(&NewCapture::text((*t).into(), None, i as i64))
                    .unwrap()
                    .id
            })
            .collect();
        (d, s, ids)
    }

    #[test]
    fn search_prefix_matches_newest_first() {
        let (_d, s, _) = seeded(&["hello world", "help me", "yellow"]);
        assert_eq!(
            previews(s.search(Some("hel"), false, 10).unwrap()),
            ["help me", "hello world"]
        );
    }

    #[test]
    fn search_tokens_are_anded() {
        let (_d, s, _) = seeded(&["hello world", "help me", "yellow"]);
        assert_eq!(
            previews(s.search(Some("wor hel"), false, 10).unwrap()),
            ["hello world"]
        );
    }

    #[test]
    fn search_ignores_case_and_diacritics() {
        let (_d, s, _) = seeded(&["Café Crème", "tea"]);
        assert_eq!(
            previews(s.search(Some("cafe"), false, 10).unwrap()),
            ["Café Crème"]
        );
        assert_eq!(
            previews(s.search(Some("CRÈME"), false, 10).unwrap()),
            ["Café Crème"]
        );
    }

    #[test]
    fn search_treats_fts_syntax_as_literal_text() {
        let (_d, s, _) = seeded(&["say \"hi\" AND bye", "near(x) col:x -x ^x *star*", "plain"]);
        for q in [
            "\"",
            "*",
            "AND",
            "NEAR(",
            "col:x",
            "-x",
            "^",
            "OR",
            "NOT",
            "(",
            ")",
            "'",
            ";",
            "' OR 1=1 --",
            "a\"b",
            "x NEAR y",
            "{x}",
            ":",
            "+",
        ] {
            let res = s.search(Some(q), false, 10);
            assert!(res.is_ok(), "{q:?}: {:?}", res.err());
        }
        // Operators match only as words, never as syntax.
        assert_eq!(
            previews(s.search(Some("AND"), false, 10).unwrap()),
            ["say \"hi\" AND bye"]
        );
        assert_eq!(
            previews(s.search(Some("col:x"), false, 10).unwrap()),
            ["near(x) col:x -x ^x *star*"]
        );
        assert_eq!(
            previews(s.search(Some("NEAR("), false, 10).unwrap()),
            ["near(x) col:x -x ^x *star*"]
        );
        // `-x` is not "NOT x": it does not return everything without x.
        assert_eq!(
            previews(s.search(Some("-x"), false, 10).unwrap()),
            ["near(x) col:x -x ^x *star*"]
        );
        // Punctuation-only input has no words to match; it is not an error.
        assert!(s.search(Some("\""), false, 10).unwrap().is_empty());
        assert!(s.search(Some("*"), false, 10).unwrap().is_empty());
    }

    #[test]
    fn search_with_nul_is_not_an_error() {
        let (_d, s, _) = seeded(&["hello world", "other"]);
        assert_eq!(
            previews(s.search(Some("hel\0wor"), false, 10).unwrap()),
            ["hello world"]
        );
        assert_eq!(s.search(Some("\0"), false, 10).unwrap().len(), 2);
        assert!(s.search(Some("\0\"\0*"), false, 10).is_ok());
    }

    #[test]
    fn empty_or_blank_query_is_the_recency_list() {
        let (_d, s, _) = seeded(&["a", "b", "c"]);
        for q in [None, Some(""), Some("   ")] {
            assert_eq!(previews(s.search(q, false, 10).unwrap()), ["c", "b", "a"]);
        }
        assert_eq!(previews(s.search(Some(""), false, 2).unwrap()), ["c", "b"]);
    }

    #[test]
    fn pinned_view_lists_and_searches_pinned_only() {
        let (_d, mut s, ids) = seeded(&["alpha one", "alpha two", "beta"]);
        s.set_pinned(&ids[0], true).unwrap();
        let pinned = s.search(None, true, 10).unwrap();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].id, ids[0]);
        assert!(pinned[0].pinned);
        assert_eq!(
            previews(s.search(Some("alp"), true, 10).unwrap()),
            ["alpha one"]
        );
        assert!(s.search(Some("beta"), true, 10).unwrap().is_empty());
        // The All view still shows everything, with the flag.
        let all = s.search(Some("alpha"), false, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[1].pinned && !all[0].pinned);
    }

    #[test]
    fn set_pinned_toggles_and_keeps_order_and_fts() {
        let (_d, mut s, ids) = seeded(&["old", "new"]);
        s.set_pinned(&ids[0], true).unwrap();
        let items = s.list_recent(10).unwrap();
        assert_eq!(
            previews(items.clone()),
            ["new", "old"],
            "pinning does not reorder"
        );
        assert!(items[1].pinned);
        assert_eq!(items[1].last_copied_at, 0);
        assert_eq!(s.search(Some("old"), false, 10).unwrap().len(), 1);
        s.set_pinned(&ids[0], false).unwrap();
        assert!(s.search(None, true, 10).unwrap().is_empty());
        assert!(matches!(
            s.set_pinned("missing", true),
            Err(CoreError::NotFound)
        ));
    }

    #[test]
    fn recopied_pinned_item_keeps_id_and_pin_and_moves_to_top() {
        let (_d, mut s, ids) = seeded(&["x", "y"]);
        s.set_pinned(&ids[0], true).unwrap();
        let again = s
            .record(&NewCapture::text("x".into(), Some("Safari".into()), 10))
            .unwrap();
        assert!(!again.inserted);
        assert_eq!(again.id, ids[0]);
        let items = s.list_recent(10).unwrap();
        assert_eq!(items[0].id, ids[0]);
        assert!(items[0].pinned);
        assert_eq!(items[0].last_copied_at, 10);
        assert_eq!(count(&s, "SELECT count(*) FROM items"), 2);
    }

    /// 10,000 items; each typical query must take under 50 ms in a release
    /// build (`cargo test --release search`). Debug builds compile SQLite
    /// unoptimised, so there the bound is relaxed and the test says so.
    #[test]
    fn search_is_fast_at_10k_items() {
        const WORDS: &[&str] = &[
            "hello",
            "world",
            "help",
            "yellow",
            "lorem",
            "ipsum",
            "dolor",
            "sit",
            "amet",
            "function",
            "return",
            "const",
            "let",
            "async",
            "await",
            "https",
            "example",
            "com",
            "invoice",
            "meeting",
            "tomorrow",
            "password",
            "address",
            "street",
            "café",
            "crème",
            "naïve",
            "résumé",
            "data",
            "error",
            "warning",
            "build",
            "release",
            "magpie",
            "clipboard",
            "search",
            "token",
            "query",
            "index",
            "table",
        ];
        let (_d, mut s) = temp_store();
        // Deterministic pseudo-random text, 5–60 words per item.
        let mut rng: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        s.conn().execute_batch("BEGIN").unwrap();
        for i in 0..10_000 {
            let n = 5 + (next() % 56) as usize;
            let mut text: Vec<String> = (0..n)
                .map(|_| WORDS[(next() % WORDS.len() as u64) as usize].to_string())
                .collect();
            text.push(format!("item{i}"));
            s.record(&NewCapture::text(text.join(" "), None, i))
                .unwrap();
        }
        s.conn().execute_batch("COMMIT").unwrap();
        for i in (0..10_000).step_by(50) {
            let id: String = s
                .conn()
                .query_row("SELECT id FROM items WHERE seq = ?1", [i + 1], |r| r.get(0))
                .unwrap();
            s.set_pinned(&id, true).unwrap();
        }

        let limit_ms = if cfg!(debug_assertions) {
            eprintln!(
                "search_is_fast_at_10k_items: debug build, bound relaxed to 250 ms; \
                 run `cargo test --release search` for the real 50 ms check"
            );
            250
        } else {
            50
        };
        let queries = [
            "",
            "h",
            "hel",
            "wor hel",
            "cafe",
            "lorem ipsum dolor",
            "item9999",
            "zzz",
            "e",
            "return const",
        ];
        let mut slowest = (0.0_f64, "", false);
        for pinned_only in [false, true] {
            for q in queries {
                s.search(Some(q), pinned_only, 500).unwrap(); // warm the cache
                for _ in 0..3 {
                    let t = std::time::Instant::now();
                    let res = s.search(Some(q), pinned_only, 500).unwrap();
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    assert!(
                        ms < limit_ms as f64,
                        "query {q:?} (pinned_only={pinned_only}) took {ms:.1} ms, {} results",
                        res.len()
                    );
                    if ms > slowest.0 {
                        slowest = (ms, q, pinned_only);
                    }
                }
            }
        }
        eprintln!(
            "search_is_fast_at_10k_items: slowest {:.2} ms for {:?} (pinned_only={}), bound {limit_ms} ms",
            slowest.0, slowest.1, slowest.2
        );
        assert_eq!(s.search(Some("item9999"), false, 500).unwrap().len(), 1);
        assert_eq!(s.search(None, true, 500).unwrap().len(), 200);
    }
}
