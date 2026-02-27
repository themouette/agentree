use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use tracing_appender::non_blocking;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use std::path::Path;

/// Keeps the non-blocking writer alive. Must be held for the daemon's entire lifetime.
/// Dropping this guard flushes and shuts down the background log writer thread.
pub struct LoggingGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Initialize structured logging to a rolling log file.
///
/// - Log format: timestamped plain text `[2026-02-27T14:32:01Z] INFO daemon started`
/// - Rotation: 10 MB per file, keep 3 rotated files (~30 MB total)
/// - Output: file only (daemon is always daemonized when this is called)
///
/// The returned `LoggingGuard` MUST be kept alive for the daemon's entire lifetime.
/// Drop it only when the daemon is about to exit.
pub fn init_logging(log_path: &Path) -> Result<LoggingGuard, String> {
    let file_appender = BasicRollingFileAppender::new(
        log_path,
        RollingConditionBasic::new().max_size(10 * 1024 * 1024), // 10 MB
        3, // keep 3 rotated files
    )
    .map_err(|e| format!("Failed to create log file appender: {}", e))?;

    let (non_blocking_file, file_guard) = non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking_file)
                .with_ansi(false)
                .with_target(false)
                .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339()),
        )
        .init();

    Ok(LoggingGuard {
        _file_guard: file_guard,
    })
}
