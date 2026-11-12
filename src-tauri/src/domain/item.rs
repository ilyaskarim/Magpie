//! Clipboard history items (AD-3, AD-4, AD-13, AD-16).

use sha2::{Digest, Sha256};

/// Maximum number of characters in a list preview (AD-16).
pub const PREVIEW_CHARS: usize = 200;

/// What an item holds. Stored as a lowercase string in the DB and the DTOs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Text,
    Html,
    Image,
    Files,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Text => "text",
            ItemKind::Html => "html",
            ItemKind::Image => "image",
            ItemKind::Files => "files",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(ItemKind::Text),
            "html" => Some(ItemKind::Html),
            "image" => Some(ItemKind::Image),
            "files" => Some(ItemKind::Files),
            _ => None,
        }
    }
}

/// The one dedupe key (AD-3): lowercase hex SHA-256 over the kind, a NUL
/// separator, then the canonical payload bytes.
pub fn dedupe_key(kind: ItemKind, payload: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_str().as_bytes());
    h.update([0u8]);
    h.update(payload);
    hex::encode(h.finalize())
}

/// A plain-text preview of at most [`PREVIEW_CHARS`] characters, with leading
/// whitespace removed.
pub fn preview(text: &str) -> String {
    text.trim_start().chars().take(PREVIEW_CHARS).collect()
}

/// Whether a copied text is worth keeping: whitespace-only copies are not.
pub fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// A capture about to be recorded. `id` is used only if no row with the same
/// `content_hash` exists yet; otherwise the existing row is bumped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCapture {
    pub id: String,
    pub kind: ItemKind,
    pub content_hash: String,
    pub plain_text: String,
    pub source_app: Option<String>,
    /// Unix milliseconds UTC (AD-4).
    pub at: i64,
}

impl NewCapture {
    pub fn text(text: String, source_app: Option<String>, at: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ItemKind::Text,
            content_hash: dedupe_key(ItemKind::Text, text.as_bytes()),
            plain_text: text,
            source_app,
            at,
        }
    }
}

/// The result of recording a capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The stable id of the row, new or existing.
    pub id: String,
    /// `true` if a new row was created, `false` if an existing one was bumped.
    pub inserted: bool,
}

/// One row of the history list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    pub id: String,
    pub kind: ItemKind,
    pub preview: String,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub last_copied_at: i64,
    pub pinned: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_key_is_stable_sha256_hex() {
        let a = dedupe_key(ItemKind::Text, b"hello");
        assert_eq!(a, dedupe_key(ItemKind::Text, b"hello"));
        assert_eq!(a.len(), 64);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn dedupe_key_depends_on_kind_and_payload() {
        let text = dedupe_key(ItemKind::Text, b"hello");
        assert_ne!(text, dedupe_key(ItemKind::Html, b"hello"));
        assert_ne!(text, dedupe_key(ItemKind::Text, b"hello "));
    }

    #[test]
    fn kind_round_trips() {
        for k in [
            ItemKind::Text,
            ItemKind::Html,
            ItemKind::Image,
            ItemKind::Files,
        ] {
            assert_eq!(ItemKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(ItemKind::parse("TEXT"), None);
    }

    #[test]
    fn preview_is_capped_in_chars_not_bytes() {
        let long = "é".repeat(1000);
        let p = preview(&long);
        assert_eq!(p.chars().count(), PREVIEW_CHARS);
        assert_eq!(preview("  \n hi"), "hi");
    }

    #[test]
    fn blank_detection() {
        assert!(is_blank("   \n\t"));
        assert!(is_blank(""));
        assert!(!is_blank(" a "));
    }

    #[test]
    fn new_text_capture_gets_uuid_and_key() {
        let c = NewCapture::text("hello".into(), Some("Notes".into()), 42);
        assert!(uuid::Uuid::parse_str(&c.id).is_ok());
        assert_eq!(c.content_hash, dedupe_key(ItemKind::Text, b"hello"));
        assert_eq!(c.kind, ItemKind::Text);
        assert_ne!(c.id, NewCapture::text("hello".into(), None, 42).id);
    }
}
