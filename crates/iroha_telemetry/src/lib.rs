//! Crate with Iroha telemetry processing
#![recursion_limit = "512"]
#[cfg(feature = "dev-telemetry")]
pub mod dev;
#[cfg(any(feature = "event-exporter", feature = "dev-telemetry"))]
mod integrity;
pub mod metrics;
pub mod privacy;
#[cfg(feature = "event-exporter")]
mod retry_period;
#[cfg(feature = "telegram")]
pub mod telegram;
#[cfg(feature = "event-exporter")]
pub mod ws;
pub use iroha_telemetry_derive::metrics;
