//! Consolidated integration-test harness for SoraFS CAR coverage.
#[path = "capacity_cli.rs"]
mod capacity_cli;
#[path = "capacity_simulation_toolkit.rs"]
mod capacity_simulation_toolkit;
#[path = "capacity_tx_stdin_cli.rs"]
mod capacity_tx_stdin_cli;
#[path = "chunk_store_cli.rs"]
mod chunk_store_cli;
#[path = "da_reconstruct_cli.rs"]
mod da_reconstruct_cli;
#[path = "fetch_cli.rs"]
mod fetch_cli;
#[path = "manifest_builder_cli.rs"]
mod manifest_builder_cli;
#[path = "provider_advert_cli.rs"]
mod provider_advert_cli;
#[path = "soranet_transport.rs"]
mod soranet_transport;
#[path = "streaming_verifier_test.rs"]
mod streaming_verifier_test;
#[path = "taikai_car_cli.rs"]
mod taikai_car_cli;
#[path = "taikai_viewer_cli.rs"]
mod taikai_viewer_cli;
#[path = "trustless_verifier.rs"]
mod trustless_verifier;
