//! Ordered schema migrations, tracked with `PRAGMA user_version`. Step `n`
//! (1-based) moves the database from version `n - 1` to `n`. Never edit a
//! step that has shipped; append a new one.

use rusqlite::Connection;

use super::db_err;
use crate::domain::CoreError;
use crate::domain::error::CoreResult;

const V1: &str = r#"
CREATE TABLE items (
    seq            INTEGER PRIMARY KEY,
    id             TEXT    NOT NULL UNIQUE,
    kind           TEXT    NOT NULL CHECK (kind IN ('text', 'html', 'image', 'files')),
    content_hash   TEXT    NOT NULL UNIQUE,
    plain_text     TEXT,
    html           TEXT,
    image_path     TEXT,
    file_paths     TEXT,
    source_app     TEXT,
    created_at     INTEGER NOT NULL,
    last_copied_at INTEGER NOT NULL,
    pinned         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX items_last_copied_at ON items (last_copied_at DESC);

CREATE VIRTUAL TABLE items_fts USING fts5(
    plain_text,
    content = 'items',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER items_fts_ai AFTER INSERT ON items BEGIN
    INSERT INTO items_fts (rowid, plain_text) VALUES (new.seq, new.plain_text);
END;

CREATE TRIGGER items_fts_ad AFTER DELETE ON items BEGIN
    INSERT INTO items_fts (items_fts, rowid, plain_text)
        VALUES ('delete', old.seq, old.plain_text);
END;

CREATE TRIGGER items_fts_au AFTER UPDATE OF plain_text ON items BEGIN
    INSERT INTO items_fts (items_fts, rowid, plain_text)
        VALUES ('delete', old.seq, old.plain_text);
    INSERT INTO items_fts (rowid, plain_text) VALUES (new.seq, new.plain_text);
END;
"#;

const MIGRATIONS: &[&str] = &[V1];

/// The schema version this build expects.
pub const LATEST: i64 = MIGRATIONS.len() as i64;

pub fn migrate(conn: &mut Connection) -> CoreResult<()> {
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db_err)?;
    if current < 0 {
        return Err(CoreError::Storage(format!(
            "the database has an invalid schema version ({current})"
        )));
    }
    if current > LATEST {
        return Err(CoreError::Storage(format!(
            "the database was created by a newer version (schema {current}, this build knows {LATEST})"
        )));
    }
    for (i, step) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = i as i64 + 1;
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute_batch(step).map_err(db_err)?;
        tx.pragma_update(None, "user_version", version)
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
    }
    Ok(())
}
