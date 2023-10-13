//! Iroha's logging utilities.
pub mod layer;
pub mod telemetry;

use std::{
    fmt::Debug,
    fs::OpenOptions,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use color_eyre::{eyre::WrapErr, Report, Result};
use iroha_config::logger::into_tracing_level;
pub use iroha_config::logger::{Configuration, ConfigurationProxy};
pub use telemetry::{Telemetry, TelemetryFields, TelemetryLayer};
use tokio::sync::mpsc::Receiver;
pub use tracing::{
    debug, debug_span, error, error_span, info, info_span, instrument as log, trace, trace_span,
    warn, warn_span, Instrument,
};
use tracing::{subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
pub use tracing_futures::Instrument as InstrumentFutures;
use tracing_subscriber::{layer::SubscriberExt, registry::Registry, reload};

/// Substrate telemetry
pub type SubstrateTelemetry = Receiver<Telemetry>;

/// Future telemetry
pub type FutureTelemetry = Receiver<Telemetry>;

/// Convenience wrapper for Telemetry types.
pub type Telemetries = (SubstrateTelemetry, FutureTelemetry);

static LOGGER_SET: AtomicBool = AtomicBool::new(false);

/// Initializes `Logger` with given [`Configuration`].
/// After the initialization `log` macros will print with the use of this `Logger`.
/// Returns the receiving side of telemetry channels (regular telemetry, future telemetry)
///
/// # Errors
/// If the logger is already set, raises a generic error.
pub fn init(configuration: &Configuration) -> Result<Option<Telemetries>> {
    if LOGGER_SET
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(None);
    }
    Ok(Some(setup_logger(configuration)?))
}

/// Disables the logger by setting `LOGGER_SET` to true. Will fail
/// if the logger has already been initialized. This function is
/// required in order to generate flamegraphs and flamecharts.
///
/// Returns true on success.
pub fn disable_logger() -> bool {
    LOGGER_SET
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn setup_logger(configuration: &Configuration) -> Result<Telemetries> {
    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(configuration.terminal_colors)
        .with_test_writer();

    if configuration.compact_mode {
        add_bunyan(configuration, layer.compact())
    } else {
        add_bunyan(configuration, layer)
    }
}

fn bunyan_writer_create(destination: PathBuf) -> Result<Arc<std::fs::File>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(destination)
        .wrap_err("Failed to create or open bunyan logs file")
        .map(Arc::new)
}

fn add_bunyan<L>(configuration: &Configuration, layer: L) -> Result<Telemetries>
where
    L: tracing_subscriber::Layer<Registry> + Debug + Send + Sync + 'static,
{
    let level: tracing::Level = into_tracing_level(configuration.max_log_level.value());
    let level_filter = tracing_subscriber::filter::LevelFilter::from_level(level);
    let (filter, handle) = reload::Layer::new(level_filter);
    configuration
        .max_log_level
        .set_handle(iroha_config::logger::ReloadHandle(handle));
    let (bunyan_layer, storage_layer) = match configuration.log_file_path.clone() {
        Some(path) => (
            Some(BunyanFormattingLayer::new(
                "bunyan_layer".into(),
                bunyan_writer_create(path)?,
            )),
            Some(JsonStorageLayer),
        ),
        None => (None, None),
    };
    let subscriber = Registry::default()
        .with(layer)
        .with(filter)
        .with(storage_layer)
        .with(tracing_error::ErrorLayer::default())
        .with(bunyan_layer);

    add_tokio_console_subscriber(configuration, subscriber)
}

fn add_tokio_console_subscriber<
    S: Subscriber + Send + Sync + 'static + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
>(
    configuration: &Configuration,
    subscriber: S,
) -> Result<Telemetries> {
    #[cfg(all(feature = "tokio-console", not(feature = "no-tokio-console")))]
    {
        let console_subscriber = console_subscriber::ConsoleLayer::builder()
            .server_addr(
                configuration
                    .tokio_console_addr
                    .parse::<std::net::SocketAddr>()
                    .expect("Invalid address for tokio console"),
            )
            .spawn();

        add_telemetry_and_set_default(configuration, subscriber.with(console_subscriber))
    }
    #[cfg(any(not(feature = "tokio-console"), feature = "no-tokio-console"))]
    {
        add_telemetry_and_set_default(configuration, subscriber)
    }
}

fn add_telemetry_and_set_default<S: Subscriber + Send + Sync + 'static>(
    configuration: &Configuration,
    subscriber: S,
) -> Result<Telemetries> {
    // static global_subscriber: dyn Subscriber = once_cell::new;
    let (subscriber, receiver, receiver_future) = TelemetryLayer::from_capacity(
        subscriber,
        configuration
            .telemetry_capacity
            .try_into()
            .expect("u32 should always fit in usize"),
    );
    set_global_default(subscriber)?;
    Ok((receiver, receiver_future))
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

    pub use tracing::{self, debug, error, info, instrument as log, span, trace, warn, Span};
}
