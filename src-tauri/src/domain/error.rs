/// The error type of everything below the shell (AD-14). It never carries
/// clipboard contents, only what went wrong and where.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("History database error: {0}")]
    Storage(String),
    #[error("Clipboard error: {0}")]
    Clipboard(String),
    #[error("This item is no longer in history")]
    NotFound,
    #[error("Paste-back error: {0}")]
    Paste(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
