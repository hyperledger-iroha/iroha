//! Shared helpers for integration tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

/// Data availability simulators shared with tooling and docs.
pub mod da;
/// Prometheus metrics parsing utilities shared by integration tests.
pub mod metrics;
/// Sandbox-aware network helpers used across integration test binaries.
pub mod sandbox;
/// Capability refusal fixtures and helpers for gateway conformance coverage.
pub mod sorafs_gateway_capability_refusal;
/// SoraFS gateway conformance harness shared between tests and tooling.
pub mod sorafs_gateway_conformance;
/// Common synchronization helpers for waiting on blocks and statuses.
pub mod sync;
