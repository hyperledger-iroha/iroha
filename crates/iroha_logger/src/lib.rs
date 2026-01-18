//! Iroha's logging utilities.
pub mod actor;
pub mod layer;
pub mod telemetry;

use std::{
    io,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use actor::LoggerHandle;
use color_eyre::{Report, Result, eyre::eyre};
pub use iroha_config::{
    logger::{Format, Level},
    parameters::actual::{DevTelemetry as DevTelemetryConfig, Logger as Config},
};
use tracing::subscriber::set_global_default;
pub use tracing::{
    Instrument, debug, debug_span, error, error_span, info, info_span, instrument as log, trace,
    trace_span, warn, warn_span,
};
pub use tracing_futures::Instrument as InstrumentFutures;
pub use tracing_subscriber::reload::Error as ReloadError;
use tracing_subscriber::{layer::SubscriberExt, registry::Registry, reload};

const TELEMETRY_CAPACITY: usize = 1000;

static LOGGER_SET: AtomicBool = AtomicBool::new(false);

/// Writer wrapper that drops broken-pipe errors so logging never aborts.
struct LossyWriter<W> {
    inner: W,
}

impl<W> LossyWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: io::Write> io::Write for LossyWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.write(buf) {
            Ok(size) => Ok(size),
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(buf.len()),
            Err(err) => Err(err),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.flush() {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(err) => Err(err),
        }
    }
}

/// MakeWriter wrapper for LossyWriter to keep fmt layers Debug-friendly.
#[derive(Clone, Copy, Debug, Default)]
struct LossyMakeWriter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LossyMakeWriter {
    type Writer = LossyWriter<std::io::Stdout>;

    fn make_writer(&'a self) -> Self::Writer {
        LossyWriter::new(std::io::stdout())
    }
}

fn try_set_logger() -> Result<()> {
    if LOGGER_SET
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(eyre!("Logger is already set."));
    }
    Ok(())
}

/// Initializes the logger globally with given [`Config`].
///
/// Returns [`LoggerHandle`] to interact with the logger instance
///
/// Works only once per process, all subsequent invocations will fail.
///
/// For usage in tests consider [`test_logger`].
///
/// # Errors
/// If the logger is already set, raises a generic error.
pub fn init_global(config: Config) -> Result<LoggerHandle> {
    try_set_logger()?;

    // Avoid TestWriter (print!/eprint!), which can panic on broken pipes during shutdown.
    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(config.terminal_colors)
        .with_writer(LossyMakeWriter);

    match config.format {
        Format::Full => step2(config, layer),
        Format::Compact => step2(config, layer.compact()),
        Format::Pretty => step2(config, layer.pretty()),
        Format::Json => step2(config, layer.json()),
    }
}

/// Returns once lazily initialised global logger for testing purposes.
///
/// Log level may be modified via `TEST_LOG_LEVEL` and `TEST_LOG_FILTER` environment variables
///
/// # Panics
/// If [`init_global`] or [`disable_global`] were called first.
pub fn test_logger() -> LoggerHandle {
    static LOGGER: OnceLock<LoggerHandle> = OnceLock::new();

    LOGGER
        .get_or_init(|| {
            // NOTE: if this config should be changed for some specific tests, consider
            // isolating those tests into a separate process and controlling default logger config
            // with ENV vars rather than by extending `test_logger` signature. This will both remain
            // `test_logger` simple and also will emphasise isolation which is necessary anyway in
            // case of singleton mocking (where the logger is the singleton).
            let config = Config {
                level: std::env::var("TEST_LOG_LEVEL")
                    .ok()
                    .and_then(|raw| raw.parse().ok())
                    .unwrap_or(Level::DEBUG),
                filter: std::env::var("TEST_LOG_FILTER")
                    .ok()
                    .and_then(|raw| raw.parse().ok()),
                format: Format::Pretty,
                terminal_colors: true,
            };

            init_global(config).expect(
                "`init_global()` or `disable_global()` should not be called before `test_logger()`",
            )
        })
        .clone()
}

/// Disables the logger globally, so that subsequent calls to [`init_global`] will fail.
///
/// Disabling logger is required in order to generate flamegraphs and flamecharts.
///
/// # Errors
/// If global logger was already initialised/disabled.
pub fn disable_global() -> Result<()> {
    try_set_logger()
}

#[allow(clippy::needless_pass_by_value)]
fn step2<L>(config: Config, layer: L) -> Result<LoggerHandle>
where
    L: tracing_subscriber::Layer<Registry> + Send + Sync + 'static,
{
    // NOTE: unfortunately, constructing EnvFilter from vector of Directive is not part of public api
    let level_filter =
        tracing_subscriber::filter::EnvFilter::try_new(config.resolve_filter().to_string())
            .expect("INTERNAL BUG: Directives not valid");
    let (level_filter, level_filter_handle) = reload::Layer::new(level_filter);
    let subscriber = Registry::default()
        .with(layer)
        .with(level_filter)
        .with(tracing_error::ErrorLayer::default());

    #[cfg(all(feature = "tokio-console", not(feature = "no-tokio-console")))]
    let subscriber = subscriber.with(console_subscriber::spawn());

    let (subscriber, receiver) = telemetry::Layer::with_capacity(subscriber, TELEMETRY_CAPACITY);
    set_global_default(subscriber)?;

    let handle = LoggerHandle::new(level_filter_handle, receiver);

    Ok(handle)
}

/// Macro for sending telemetry info
#[macro_export]
macro_rules! telemetry_target {
    () => {
        concat!("telemetry::", module_path!())
    };
}

/// Macro for sending telemetry info
#[macro_export]
macro_rules! telemetry {
    () => {
        $crate::info!(target: iroha_logger::telemetry_target!(),)
    };
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            $($k).+ = $($field)*
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            ?$($k).+ = $($field)*
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            %$($k).+ = $($field)*
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            $($k).+, $($field)*
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            ?$($k).+, $($field)*
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            %$($k).+, $($field)*
        )
    );
    (?$($k:ident).+) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            ?$($k).+
        )
    );
    (%$($k:ident).+) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            %$($k).+
        )
    );
    ($($k:ident).+) => (
        $crate::info!(
            target: iroha_logger::telemetry_target!(),
            $($k).+
        )
    );
}

/// Macro for getting telemetry future target
#[macro_export]
macro_rules! telemetry_future_target {
    () => {
        concat!("telemetry_future::", module_path!())
    };
}

/// Macro for sending telemetry future info
#[macro_export]
macro_rules! telemetry_future {
    // All arguments match arms are from info macro
    () => {
        $crate::info!(target: iroha_logger::telemetry_future_target!(),)
    };
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            $($k).+ = $($field)*
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            ?$($k).+ = $($field)*
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            %$($k).+ = $($field)*
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            $($k).+, $($field)*
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            ?$($k).+, $($field)*
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            %$($k).+, $($field)*
        )
    );
    (?$($k:ident).+) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            ?$($k).+
        )
    );
    (%$($k:ident).+) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            %$($k).+
        )
    );
    ($($k:ident).+) => (
        $crate::info!(
            target: iroha_logger::telemetry_future_target!(),
            $($k).+
        )
    );
}

/// Installs the panic hook with [`color_eyre::install`] if it isn't installed yet
///
/// # Errors
/// Fails if [`color_eyre::install`] fails
pub fn install_panic_hook() -> Result<(), Report> {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        color_eyre::install()
    } else {
        Ok(())
    }
}

pub mod prelude {
    //! Module with most used items. Needs to be imported when using `log` macro to avoid `tracing` crate dependency

    pub use tracing::{self, Span, debug, error, info, instrument as log, span, trace, warn};
}

#[cfg(test)]
mod tests {
    use super::{LossyMakeWriter, LossyWriter};
    use std::io::{self, Write};
    use tracing_subscriber::fmt::MakeWriter;

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    #[test]
    fn lossy_writer_ignores_broken_pipe() {
        let mut writer = LossyWriter::new(BrokenPipeWriter);
        let buf = b"logger";
        let written = writer.write(buf).expect("write should ignore broken pipe");
        assert_eq!(written, buf.len());
        writer.flush().expect("flush should ignore broken pipe");
    }

    #[test]
    fn lossy_make_writer_builds() {
        let _ = LossyMakeWriter.make_writer();
        let _ = format!("{LossyMakeWriter:?}");
    }
}
