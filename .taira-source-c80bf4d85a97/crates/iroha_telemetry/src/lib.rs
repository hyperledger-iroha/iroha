//! Crate with Iroha telemetry processing
#![recursion_limit = "512"]

#[cfg(feature = "dev-telemetry")]
pub mod dev;
pub mod futures;
mod integrity;
pub mod metrics;
pub mod privacy;
mod retry_period;
#[cfg(feature = "telegram")]
pub mod telegram;
pub mod ws;

/// Re-export Norito JSON derive macros for telemetry crate usage.
pub mod json_macros {
    pub use norito::derive::{JsonDeserialize, JsonSerialize};
}

pub use iroha_config::parameters::actual::{
    DevTelemetry as DevTelemetryConfig, Telemetry as TelemetryConfig,
};
pub use iroha_telemetry_derive::metrics;

pub mod msg {
    //! Messages that can be sent to the telemetry

    /// The message that is sent to the telemetry when the node is initialized
    pub const SYSTEM_CONNECTED: &str = "system.connected";
}
