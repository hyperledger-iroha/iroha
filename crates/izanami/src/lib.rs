//! Reusable Izanami fault-injection and local genesis-orchestration helpers.

pub mod communication_vulnerabilities;
pub mod faults;

/// Local genesis preparation and startup-validation helpers.
pub use iroha_test_network::genesis_support;
