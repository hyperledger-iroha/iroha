//! Consolidated integration-test harness for FASTPQ prover coverage.
#[cfg(feature = "dev-tools")]
#[path = "backend_regression.rs"]
mod backend_regression;
#[path = "common/mod.rs"]
mod common;
#[path = "document_examples.rs"]
mod document_examples;
#[path = "packing.rs"]
mod packing;
#[path = "poseidon_manifest_consistency.rs"]
mod poseidon_manifest_consistency;
#[cfg(feature = "dev-tools")]
#[path = "proof_fixture.rs"]
mod proof_fixture;
#[path = "realistic_flows.rs"]
mod realistic_flows;
#[path = "trace_commitment.rs"]
mod trace_commitment;
