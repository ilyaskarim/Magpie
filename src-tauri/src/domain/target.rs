/// An application Magpie may paste into: the one that was frontmost when the
/// history window was shown (AD-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetApp {
    /// OS process id.
    pub pid: i64,
    /// Bundle identifier (macOS), checked before activation so a reused pid
    /// never sends the paste to an unrelated app.
    pub bundle_id: Option<String>,
    /// User-facing name, if known. Never needed to paste; useful in logs.
    pub name: Option<String>,
}
