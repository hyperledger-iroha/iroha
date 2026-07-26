//! Grouped Torii protocol, MCP, Norito ingress, and WebSocket tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../connect_gating.rs"]
mod connect_gating;
#[path = "../mcp_endpoints.rs"]
mod mcp_endpoints;
#[path = "../norito_ingress.rs"]
mod norito_ingress;
#[path = "../p2p_ws.rs"]
mod p2p_ws;
#[path = "../ws_proof_integration.rs"]
mod ws_proof_integration;
#[path = "../ws_proof_json_mapping.rs"]
mod ws_proof_json_mapping;
