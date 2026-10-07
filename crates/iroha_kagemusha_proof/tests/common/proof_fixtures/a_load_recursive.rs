//! Shared genuine proof builder; contains no registered test cases.
//! Genuine rooted Bootstrap predecessor and Load recursive partition inventory.
//! A passing diagnostic is not a final Load lineage acceptance or size gate.

// Both included suites also run independently; each keeps its own deterministic
// helper/cache module when composed into this complete-chain test.
#![allow(clippy::duplicate_mod)]

#[path = "../bootstrap.rs"]
mod bootstrap;
#[path = "../bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
/// Shared genuine outer-proof fixture, including its component regressions.
#[path = "bootstrap_omega.rs"]
pub mod bootstrap_outer;
#[path = "../mod.rs"]
mod common;
#[path = "../consuming_proof.rs"]
mod consuming_proof;
/// Shared genuine Load sigma, map and signed-object fixtures.
#[path = "a_load.rs"]
pub mod load_components;
#[path = "../load_objects.rs"]
#[allow(dead_code)]
mod load_objects;

include!("a_load_recursive_body.rs");
