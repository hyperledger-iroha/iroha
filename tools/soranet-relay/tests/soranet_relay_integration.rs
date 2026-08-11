//! Consolidated integration-test harness for the SoraNet relay.

#[path = "adaptive_and_puzzle.rs"]
mod adaptive_and_puzzle;
#[path = "constant_rate_handshake.rs"]
mod constant_rate_handshake;
#[path = "vpn_adapter.rs"]
mod vpn_adapter;
#[path = "vpn_config.rs"]
mod vpn_config;
#[path = "vpn_end_to_end.rs"]
mod vpn_end_to_end;
#[path = "vpn_overlay.rs"]
mod vpn_overlay;
#[path = "vpn_runtime.rs"]
mod vpn_runtime;
