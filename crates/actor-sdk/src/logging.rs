//! Actor Logging

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Debug
    Debug,
    /// Info
    Info,
    /// Warning
    Warn,
    /// Error
    Error,
}

/// Log a message from the actor
pub fn log(level: LogLevel, message: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        // Host call would go here via WASI
        let _ = (level, message);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "std"))]
    {
        let level_str = match level {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        };
        eprintln!("[{}] {}", level_str, message);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "std")))]
    {
        // No-op in no_std without WASM
        let _ = (level, message);
    }
}

/// Log debug message
pub fn debug(message: &str) {
    log(LogLevel::Debug, message);
}

/// Log info message
pub fn info(message: &str) {
    log(LogLevel::Info, message);
}

/// Log warning message
pub fn warn(message: &str) {
    log(LogLevel::Warn, message);
}

/// Log error message
pub fn error(message: &str) {
    log(LogLevel::Error, message);
}
