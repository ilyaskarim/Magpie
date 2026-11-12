use crate::domain::error::CoreResult;
use crate::domain::{ItemSummary, NewCapture, Recorded};

/// Persistent clipboard history.
pub trait Store {
    /// Inserts `capture`, or, if an item with the same `content_hash` exists,
    /// bumps its `last_copied_at` and `source_app` and keeps its `id` and
    /// `pinned` (AD-3).
    fn record(&mut self, capture: &NewCapture) -> CoreResult<Recorded>;

    /// The most recent items, newest `last_copied_at` first (AD-4).
    fn list_recent(&self, limit: usize) -> CoreResult<Vec<ItemSummary>>;
}

#[cfg(test)]
pub mod fakes {
    use super::*;
    use crate::domain::item::preview;

    #[derive(Default)]
    pub struct MemoryStore {
        rows: Vec<(String, ItemSummary)>, // (content_hash, summary)
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
            Ok(Recorded {
                id: c.id.clone(),
                inserted: true,
            })
        }

        fn list_recent(&self, limit: usize) -> CoreResult<Vec<ItemSummary>> {
            // Newest insert first on ties, like SQLite's `seq DESC`.
            let mut v: Vec<_> = self.rows.iter().rev().map(|(_, r)| r.clone()).collect();
            v.sort_by_key(|r| std::cmp::Reverse(r.last_copied_at)); // stable
            v.truncate(limit);
            Ok(v)
        }
    }
}
