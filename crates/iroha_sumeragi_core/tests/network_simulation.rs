//! Package-harness entry point for the production reducer simulations.

pub use iroha_sumeragi_core::*;

#[path = "../../iroha_core/src/sumeragi/v2_core/network_simulation.rs"]
mod production_network_simulation;
