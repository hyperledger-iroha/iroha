#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Grouped integration harness for Nexus, streaming, and Sora/SoraFS flows.

#[path = "byte_merkle_cross.rs"]
mod byte_merkle_cross;
#[path = "byte_merkle_path.rs"]
mod byte_merkle_path;
#[path = "ivm_header_decode.rs"]
mod ivm_header_decode;
#[path = "ivm_header_smoke.rs"]
mod ivm_header_smoke;
#[path = "kotodama_examples.rs"]
mod kotodama_examples;
#[path = "merkle_unified.rs"]
mod merkle_unified;
#[path = "nexus/mod.rs"]
mod nexus;
#[path = "norito_burn_fixture.rs"]
mod norito_burn_fixture;
#[path = "norito_streaming_end_to_end.rs"]
mod norito_streaming_end_to_end;
#[path = "norito_streaming_fec.rs"]
mod norito_streaming_fec;
#[path = "norito_streaming_feedback.rs"]
mod norito_streaming_feedback;
#[path = "norito_streaming_negative.rs"]
mod norito_streaming_negative;
#[path = "norito_streaming_roundtrip.rs"]
mod norito_streaming_roundtrip;
#[path = "sora_parliament_lifecycle_smoke.rs"]
mod sora_parliament_lifecycle_smoke;
#[path = "sora_runtime_upgrade_resilience.rs"]
mod sora_runtime_upgrade_resilience;
#[path = "sorafs_gateway_capability_refusal.rs"]
mod sorafs_gateway_capability_refusal;
#[path = "sorafs_gateway_conformance.rs"]
mod sorafs_gateway_conformance;
#[path = "sorafs_orchestrator_parity.rs"]
mod sorafs_orchestrator_parity;
#[path = "sorafs_reconciliation.rs"]
mod sorafs_reconciliation;
#[path = "soranet_web_deploy.rs"]
mod soranet_web_deploy;
#[path = "streaming/mod.rs"]
mod streaming;
