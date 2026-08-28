//! Integration test suite for `iroha_p2p`.
//!
//! Groups end-to-end scenarios under the `integration` module.
mod integration;

#[path = "production_source_reachability.rs"]
mod production_source_reachability;

#[path = "retired_relay_surface.rs"]
mod retired_relay_surface;
