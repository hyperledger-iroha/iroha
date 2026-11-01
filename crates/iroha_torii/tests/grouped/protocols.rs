//! Grouped Torii protocol, MCP, Norito ingress, and WebSocket tests.

#[path = "../../src/build_identity_test_fixture.rs"]
mod build_identity_test_fixture;
#[path = "../fixtures.rs"]
mod fixtures;
#[path = "../common/norito_rpc_harness.rs"]
mod norito_rpc_harness;
#[path = "../common/proof_events.rs"]
mod proof_events;

#[path = "../connect_gating.rs"]
mod connect_gating;
#[path = "../mcp_endpoints.rs"]
mod mcp_endpoints;
#[path = "../norito_ingress.rs"]
mod norito_ingress;
#[path = "../ws_proof_integration.rs"]
mod ws_proof_integration;
#[path = "../ws_proof_json_mapping.rs"]
mod ws_proof_json_mapping;
