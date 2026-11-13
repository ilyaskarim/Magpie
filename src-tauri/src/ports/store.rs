use crate::domain::error::CoreResult;
use crate::domain::{ItemSummary, NewCapture, Recorded};

/// Persistent clipboard history.
pub trait Store {
    /// Inserts `capture`, or, if an item with the same `content_hash` exists,
    /// bumps its `last_copied_at` and `source_app` and keeps its `id` and
    /// `pinned` (AD-3).
    fn record(&mut self, capture: &NewCapture) -> CoreResult<Recorded>;

    /// The most recent items, newest `last_copied_at` first (AD-4). The UI
    /// uses `search` with an empty query; this shorthand serves tests.
    #[cfg(test)]
    fn list_recent(&self, limit: usize) -> CoreResult<Vec<ItemSummary>> {
        self.search(None, false, limit)
    }

    /// Items matching `query` (free text as the user typed it; every token
    /// must prefix-match a word, AD-8), optionally pinned ones only, ordered
    /// by `last_copied_at` then insertion, newest first. `None` or a blank
    /// query lists by recency.
    fn search(
        &self,
        query: Option<&str>,
        pinned_only: bool,
        limit: usize,
    ) -> CoreResult<Vec<ItemSummary>>;

    /// Pins or unpins an item; nothing else changes.
    /// [`CoreError::NotFound`] if no item has this id.
    ///
    /// [`CoreError::NotFound`]: crate::domain::CoreError::NotFound
    fn set_pinned(&mut self, id: &str, pinned: bool) -> CoreResult<()>;

    /// The full plain text of an item. [`CoreError::NotFound`] if no item
    /// has this id.
    ///
    /// [`CoreError::NotFound`]: crate::domain::CoreError::NotFound
    fn get_text(&self, id: &str) -> CoreResult<String>;

    /// Moves an item to the top: sets `last_copied_at` to `at` and changes
    /// nothing else (AD-7). [`CoreError::NotFound`] if no item has this id.
    ///
    /// [`CoreError::NotFound`]: crate::domain::CoreError::NotFound
    fn touch(&mut self, id: &str, at: i64) -> CoreResult<()>;
}

#[cfg(test)]
pub mod fakes {
    use super::*;
    use crate::domain::CoreError;
    use crate::domain::item::preview;

    #[derive(Default)]
    pub struct MemoryStore {
        rows: Vec<(String, ItemSummary)>, // (content_hash, summary)
        texts: Vec<(String, String)>,     // (id, full plain text)
    }

    impl Store for MemoryStore {
        fn record(&mut self, c: &NewCapture) -> CoreResult<Recorded> {
            if let Some((_, row)) = self.rows.iter_mut().find(|(h, _)| *h == c.content_hash) {
                row.last_copied_at = c.at;
                if c.source_app.is_some() {
                    row.source_app = c.source_app.clone();
                }
                return Ok(Recorded {
                    id: row.id.clone(),
                    inserted: false,
                });
            }
            self.rows.push((
                c.content_hash.clone(),
                ItemSummary {
                    id: c.id.clone(),
                    kind: c.kind,
                    preview: preview(&c.plain_text),
                    source_app: c.source_app.clone(),
                    created_at: c.at,
                    last_copied_at: c.at,
                    pinned: false,
                },
            ));
            self.texts.push((c.id.clone(), c.plain_text.clone()));
            Ok(Recorded {
                id: c.id.clone(),
                inserted: true,
            })
        }

        fn search(
            &self,
            query: Option<&str>,
            pinned_only: bool,
            limit: usize,
        ) -> CoreResult<Vec<ItemSummary>> {
            // A simple stand-in for FTS5: case-insensitive, every token must
            // prefix some word of the full text.
            // Query and text are split the same way, on anything not
            // alphanumeric, like SQLite's unicode61 (so `col:x` is `col` AND
            // `x`). Unlike SQLite, the fake does not fold diacritics.
            let words_of = |t: &str| -> Vec<String> {
                t.to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| !w.is_empty())
                    .map(str::to_owned)
                    .collect()
            };
            let tokens = words_of(query.unwrap_or(""));
            let matches = |id: &str| {
                let words = self
                    .texts
                    .iter()
                    .find(|(i, _)| i == id)
                    .map(|(_, t)| words_of(t))
                    .unwrap_or_default();
                tokens
                    .iter()
                    .all(|t| words.iter().any(|w| w.starts_with(t.as_str())))
            };
            // Newest insert first on ties, like SQLite's `seq DESC`.
            let mut v: Vec<_> = self
                .rows
                .iter()
                .rev()
                .map(|(_, r)| r)
                .filter(|r| !pinned_only || r.pinned)
                .filter(|r| matches(&r.id))
                .cloned()
                .collect();
            v.sort_by_key(|r| std::cmp::Reverse(r.last_copied_at)); // stable
            v.truncate(limit);
            Ok(v)
        }

        fn get_text(&self, id: &str) -> CoreResult<String> {
            self.texts
                .iter()
                .find(|(i, _)| i == id)
                .map(|(_, t)| t.clone())
                .ok_or(CoreError::NotFound)
        }

        fn touch(&mut self, id: &str, at: i64) -> CoreResult<()> {
            let (_, row) = self
                .rows
                .iter_mut()
                .find(|(_, r)| r.id == id)
                .ok_or(CoreError::NotFound)?;
            row.last_copied_at = at;
            Ok(())
        }

        fn set_pinned(&mut self, id: &str, pinned: bool) -> CoreResult<()> {
            let (_, row) = self
                .rows
                .iter_mut()
                .find(|(_, r)| r.id == id)
                .ok_or(CoreError::NotFound)?;
            row.pinned = pinned;
            Ok(())
        }
    }

    #[test]
    fn fake_search_prefix_and_pins() {
        let mut s = MemoryStore::default();
        let hello = s
            .record(&NewCapture::text("Hello world".into(), None, 1))
            .unwrap();
        s.record(&NewCapture::text("yellow".into(), None, 2))
            .unwrap();
        let ids = |v: Vec<ItemSummary>| v.into_iter().map(|i| i.id).collect::<Vec<_>>();
        assert_eq!(
            ids(s.search(Some("wor hel"), false, 10).unwrap()),
            std::slice::from_ref(&hello.id)
        );
        assert_eq!(s.search(Some("  "), false, 10).unwrap().len(), 2);
        assert!(s.search(None, true, 10).unwrap().is_empty());
        // Punctuation splits query tokens as it splits stored text.
        assert_eq!(
            ids(s.search(Some("hello:wor"), false, 10).unwrap()),
            std::slice::from_ref(&hello.id)
        );
        s.set_pinned(&hello.id, true).unwrap();
        assert_eq!(
            ids(s.search(None, true, 10).unwrap()),
            std::slice::from_ref(&hello.id)
        );
        assert!(matches!(
            s.set_pinned("missing", true),
            Err(CoreError::NotFound)
        ));
    }
}
