//! Non-vacuous four-validator Exact12 privacy release harness.
//!
//! This target is the sole release-authoritative Cargo owner of all seven
//! Exact12 network modules.
//!
//! Cargo only builds this target when both `zk-stark` and the non-shipping
//! `privacy-release-evidence` fixture builders are enabled. Keeping every
//! Exact12 network module unconditional inside this feature-gated target makes
//! a missing release feature a Cargo error instead of a successful zero-test
//! qualification run.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "privacy_exact12_activation_network.rs"]
mod privacy_exact12_activation_network;
#[path = "privacy_exact12_jindo_network.rs"]
mod privacy_exact12_jindo_network;
#[path = "privacy_exact12_orchard_pq_masp_network.rs"]
mod privacy_exact12_orchard_pq_masp_network;
#[path = "privacy_exact12_retained_network.rs"]
mod privacy_exact12_retained_network;
#[path = "privacy_exact12_zk_ams_vega_network.rs"]
mod privacy_exact12_zk_ams_vega_network;
#[path = "privacy_exact12_zk_x509_network.rs"]
mod privacy_exact12_zk_x509_network;
#[path = "zk_ace_localnet.rs"]
mod zk_ace_localnet;
