//! Iroha configuration and related utilities.
pub use iroha_config_base as base;
use log::LevelFilter;
use thiserror::Error;
pub mod client_api;
pub mod kura;
pub mod logger;
pub mod parameters;
pub mod snapshot;
/// Enables verbose tracing of configuration loading.
///
/// This installs a minimal `log` logger that prints only messages originating from modules under
/// `iroha_config_base::*` to stderr at `TRACE` level. It is used early (before the global tracing
/// subscriber is set) to observe config parsing behavior when `--trace-config` is passed.
///
/// # Errors
/// Returns an error if a global logger is already installed via `log`.
#[derive(Debug, Error, Copy, Clone)]
#[error("failed to set logger")]
pub struct LoggerSetupError;
struct ConfigTraceLogger;
impl log::Log for ConfigTraceLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target().starts_with("iroha_config_base")
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
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
static CONFIG_TRACE_LOGGER: ConfigTraceLogger = ConfigTraceLogger;
/// Enable early tracing output for configuration parsing.
///
/// # Errors
///
/// Returns `LoggerSetupError` if a global logger is already installed via `log`.
pub fn enable_tracing() -> Result<(), LoggerSetupError> {
    #[cfg(target_has_atomic = "ptr")]
    {
        log::set_logger(&CONFIG_TRACE_LOGGER).map_err(|_| LoggerSetupError)?;
    }
    #[cfg(not(target_has_atomic = "ptr"))]
    {
        return Err(LoggerSetupError);
    }
    log::set_max_level(LevelFilter::Trace);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{CONFIG_TRACE_LOGGER, enable_tracing};
    use log::Log;
    #[test]
    fn logger_filters_by_module_prefix() {
        let allow = log::MetadataBuilder::new()
            .target("iroha_config_base::read")
            .level(log::Level::Trace)
            .build();
        let deny = log::MetadataBuilder::new()
            .target("some_other_crate::mod")
            .level(log::Level::Trace)
            .build();
        assert!(CONFIG_TRACE_LOGGER.enabled(&allow));
        assert!(!CONFIG_TRACE_LOGGER.enabled(&deny));
    }
    #[test]
    fn enable_tracing_sets_logger_once() {
        enable_tracing().expect("first call succeeds");
        assert!(enable_tracing().is_err());
    }
}
