//! Grouped `iroha_core_zk` integration tests, linked as one binary.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[path = "zk_ipa_native.rs"]
mod zk_ipa_native;
#[path = "zk_preverify_budget.rs"]
mod zk_preverify_budget;
