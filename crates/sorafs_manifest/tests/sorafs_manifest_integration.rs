//! Consolidated integration-test harness for SoraFS manifests.
#[path = "chunker_manifest.rs"]
mod chunker_manifest;
#[path = "discovery_propagation.rs"]
mod discovery_propagation;
#[path = "governance_proofs.rs"]
mod governance_proofs;
#[path = "orderbook_fixtures.rs"]
mod orderbook_fixtures;
#[path = "pdp.rs"]
mod pdp;
#[path = "pdp_fixtures.rs"]
mod pdp_fixtures;
#[path = "pdp_generator_cli.rs"]
mod pdp_generator_cli;
#[path = "por_fixtures.rs"]
mod por_fixtures;
#[path = "provider_admission_fixtures.rs"]
mod provider_admission_fixtures;
#[path = "replication_order_fixtures.rs"]
mod replication_order_fixtures;
#[path = "sorafs_validate_cli.rs"]
mod sorafs_validate_cli;
