//! Grouped Iroha Core integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../zk_roots_get_cap.rs"]
mod zk_roots_get_cap;
#[path = "../zk_shield_transfer_audit.rs"]
mod zk_shield_transfer_audit;
#[path = "../zk_stark.rs"]
mod zk_stark;
#[path = "../zk_testkit.rs"]
mod zk_testkit;
#[path = "../zk_verify.rs"]
mod zk_verify;
#[path = "../zk_verify_vendor_e2e.rs"]
mod zk_verify_vendor_e2e;
#[path = "../zk_vk_circuit_index.rs"]
mod zk_vk_circuit_index;
#[path = "../zk_vk_events.rs"]
mod zk_vk_events;
#[path = "../zk_vote_get_tally.rs"]
mod zk_vote_get_tally;
#[path = "../zk_vote_tally_audit.rs"]
mod zk_vote_tally_audit;
