//! Use cases that combine ports. Cfg-free and Tauri-free.

pub mod capture;
pub mod paste;

/// Now as Unix milliseconds UTC (AD-4).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
