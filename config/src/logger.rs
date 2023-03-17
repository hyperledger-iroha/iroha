//! Module containing logic related to spawning a logger from the
//! configuration, as well as run-time reloading of the log-level.
#![allow(clippy::std_instead_of_core)]
use core::fmt::Debug;

use derive_more::{Deref, DerefMut};
use iroha_config_base::{
    derive::{Documented, Proxy},
    runtime_upgrades::{handle, ReloadError, ReloadMut},
};
use serde::{Deserialize, Serialize};
use strum::FromRepr;
use tracing::Subscriber;
use tracing_subscriber::{filter::LevelFilter, reload::Handle};

const TELEMETRY_CAPACITY: u32 = 1000;
const DEFAULT_COMPACT_MODE: bool = false;
const DEFAULT_TERMINAL_COLORS: bool = true;

/// Log level for reading from environment and (de)serializing
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, FromRepr,
)]
#[allow(clippy::upper_case_acronyms)]
#[repr(u8)]
pub enum Level {
    /// Trace
    TRACE,
    /// Debug
    DEBUG,
    /// Info (Default)
    #[default]
    INFO,
    /// Warn
    WARN,
    /// Error
    ERROR,
}

impl From<Level> for tracing::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::TRACE => Self::TRACE,
            Level::DEBUG => Self::DEBUG,
            Level::INFO => Self::INFO,
            Level::WARN => Self::WARN,
            Level::ERROR => Self::ERROR,
        }
    }
}

impl<T: Subscriber + Debug> ReloadMut<Level> for Handle<LevelFilter, T> {
    fn reload(&mut self, level: Level) -> Result<(), ReloadError> {
        let level_filter = tracing_subscriber::filter::LevelFilter::from_level(level.into());

        Handle::reload(self, level_filter).map_err(|err| {
            if err.is_dropped() {
                ReloadError::Dropped
            } else {
                ReloadError::Poisoned
            }
        })
    }
}

/// Wrapper around [`Level`] for runtime upgrades.
#[derive(Debug, Clone, Default, Deref, DerefMut, Deserialize, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct SyncLevel(handle::SyncValue<Level, handle::Singleton<Level>>);

impl From<Level> for SyncLevel {
    fn from(level: Level) -> Self {
        Self(level.into())
    }
}

impl PartialEq for SyncLevel {
    fn eq(&self, other: &Self) -> bool {
        self.0.value() == other.0.value()
    }
}

impl Eq for SyncLevel {}

/// 'Logger' configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Proxy, Documented)]
#[serde(rename_all = "UPPERCASE")]
pub struct Configuration {
    /// Maximum log level
    #[config(serde_as_str)]
    pub max_log_level: SyncLevel,
    /// Capacity (or batch size) for telemetry channel
    pub telemetry_capacity: u32,
    /// Compact mode (no spans from telemetry)
    pub compact_mode: bool,
    /// If provided, logs will be copied to said file in the
    /// format readable by [bunyan](https://lib.rs/crates/bunyan)
    #[config(serde_as_str)]
    pub log_file_path: Option<std::path::PathBuf>,
    /// Enable ANSI terminal colors for formatted output.
    pub terminal_colors: bool,
}

impl Default for ConfigurationProxy {
    fn default() -> Self {
        Self {
            max_log_level: Some(SyncLevel::default()),
            telemetry_capacity: Some(TELEMETRY_CAPACITY),
            compact_mode: Some(DEFAULT_COMPACT_MODE),
            log_file_path: Some(None),
            terminal_colors: Some(DEFAULT_TERMINAL_COLORS),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use proptest::prelude::*;

    use super::*;

    prop_compose! {
        pub fn arb_proxy()
        (max_log_level in prop::option::of(Just(SyncLevel::default())),
        telemetry_capacity in prop::option::of(Just(TELEMETRY_CAPACITY)),
        compact_mode in prop::option::of(Just(DEFAULT_COMPACT_MODE)),
        log_file_path in prop::option::of(Just(None)),
        terminal_colors in prop::option::of(Just(DEFAULT_TERMINAL_COLORS))) -> ConfigurationProxy {
            ConfigurationProxy { max_log_level, telemetry_capacity, compact_mode, log_file_path, terminal_colors }
        }
    }
}
