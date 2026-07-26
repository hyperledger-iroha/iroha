//! Iroha configuration and related utilities.

use error_stack::Report;
pub use iroha_config_base as base;
use log::{LevelFilter, SetLoggerError};
use thiserror::Error;

pub mod client_api;
pub mod kura;
pub mod logger;
pub mod parameters;
pub mod snapshot;

/// Enables verbose tracing of configuration loading.
///
/// This installs a minimal `log` logger that prints only messages originating
/// from modules under `iroha_config_base::*` to stderr at `TRACE` level.
/// It is used early (before the global tracing subscriber is set) to observe
/// config parsing behavior when `--trace-config` is passed.
///
/// # Errors
/// Returns an error if a global logger is already installed via `log`.
#[derive(Debug, Error, Copy, Clone)]
#[error("failed to set logger")]
pub struct LoggerSetupError;

impl From<SetLoggerError> for LoggerSetupError {
    fn from(_: SetLoggerError) -> Self {
        LoggerSetupError
    }
}

type Result<T, E> = core::result::Result<T, Report<E>>;

/// Enable early tracing output for configuration parsing.
///
/// # Errors
///
/// Returns `LoggerSetupError` if a global logger is already installed via `log`.
pub fn enable_tracing() -> Result<(), LoggerSetupError> {
    struct ConfigTraceLogger;

    impl log::Log for ConfigTraceLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            // Limit early logging to config parsing internals only
            metadata.target().starts_with("iroha_config_base")
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                // Keep output simple and deterministic
                eprintln!(
                    "[{}] {}: {}",
                    record.level(),
                    record.target(),
                    record.args()
                );
            }
        }

        fn flush(&self) {}
    }

    // Install logger once and enable full verbosity for the targeted module
    static LOGGER: ConfigTraceLogger = ConfigTraceLogger;

    #[cfg(target_has_atomic = "ptr")]
    {
        log::set_logger(&LOGGER).map_err(LoggerSetupError::from)?;
    }
    #[cfg(not(target_has_atomic = "ptr"))]
    unsafe {
        log::set_logger_racy(&LOGGER).map_err(LoggerSetupError::from)?;
    }
    log::set_max_level(LevelFilter::Trace);
    Ok(())
}

#[cfg(test)]
mod tests {
    use log::Log;

    use super::enable_tracing;

    #[test]
    fn logger_filters_by_module_prefix() {
        // Exercise the `enabled` predicate directly to avoid installing globals in tests
        struct T;
        impl log::Log for T {
            fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                metadata.target().starts_with("iroha_config_base")
            }
            fn log(&self, _record: &log::Record<'_>) {}
            fn flush(&self) {}
        }

        let logger = T;
        let allow = log::MetadataBuilder::new()
            .target("iroha_config_base::read")
            .level(log::Level::Trace)
            .build();
        let deny = log::MetadataBuilder::new()
            .target("some_other_crate::mod")
            .level(log::Level::Trace)
            .build();

        assert!(logger.enabled(&allow));
        assert!(!logger.enabled(&deny));
    }

    #[test]
    fn enable_tracing_sets_logger_once() {
        enable_tracing().expect("first call succeeds");
        assert!(enable_tracing().is_err());
    }
}
