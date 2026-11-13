//! Turning what the user typed into an FTS5 query (AD-8).

/// Builds an FTS5 `MATCH` expression from free text, or `None` if the input
/// has no tokens (the caller then lists by recency).
///
/// Each token separated by whitespace or NUL (which would otherwise cut the
/// FTS5 string short) becomes a quoted prefix term, `"tok"*`,
/// with any `"` inside it doubled; terms are joined by spaces, which FTS5
/// treats as AND. Because every token is a quoted string, FTS5 operators
/// (`AND`, `OR`, `NOT`, `NEAR(`, `col:`, `-`, `^`, `*`, parentheses) in the
/// input are only ever literal text. The result is passed as a bound
/// parameter, never spliced into SQL.
pub fn fts_query(user_input: &str) -> Option<String> {
    let terms: Vec<String> = user_input
        .split(|c: char| c.is_whitespace() || c == '\0')
        .filter(|tok| !tok.is_empty())
        .map(|tok| format!("\"{}\"*", tok.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_whitespace_are_none() {
        assert_eq!(fts_query(""), None);
        assert_eq!(fts_query("   "), None);
        assert_eq!(fts_query("\t\n "), None);
    }

    #[test]
    fn tokens_become_quoted_prefix_terms() {
        assert_eq!(fts_query("hel").as_deref(), Some("\"hel\"*"));
        assert_eq!(
            fts_query("  wor   hel ").as_deref(),
            Some("\"wor\"* \"hel\"*")
        );
    }

    #[test]
    fn nul_separates_tokens() {
        assert_eq!(fts_query("\0").as_deref(), None);
        assert_eq!(fts_query("a\0b").as_deref(), Some("\"a\"* \"b\"*"));
    }

    #[test]
    fn fts_syntax_is_quoted_as_literal_text() {
        let cases = [
            ("\"", "\"\"\"\"*"),
            ("*", "\"*\"*"),
            ("AND", "\"AND\"*"),
            ("NEAR(", "\"NEAR(\"*"),
            ("col:x", "\"col:x\"*"),
            ("-x", "\"-x\"*"),
            ("^", "\"^\"*"),
            ("a\"b", "\"a\"\"b\"*"),
            ("a OR b", "\"a\"* \"OR\"* \"b\"*"),
        ];
        for (input, expected) in cases {
            assert_eq!(fts_query(input).as_deref(), Some(expected), "{input}");
        }
    }
}
