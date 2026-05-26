//! Shared helpers for integration tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

/// Shared binary lookup and env-override helpers for CLI integration tests.
pub mod binary_resolver;
/// Data availability simulators shared with tooling and docs.
pub mod da;
/// Bounded HTTP helpers for integration tests.
pub mod http;
/// Shared binary resolution helpers for `kagami`-driven localnet tests.
pub mod kagami;
/// Prometheus metrics parsing utilities shared by integration tests.
pub mod metrics;
/// Bounded process helpers for integration tests.
pub mod process;
/// Sandbox-aware network helpers used across integration test binaries.
pub mod sandbox;
/// Capability refusal fixtures and helpers for gateway conformance coverage.
pub mod sorafs_gateway_capability_refusal;
/// SoraFS gateway conformance harness shared between tests and tooling.
pub mod sorafs_gateway_conformance;
/// Common synchronization helpers for waiting on blocks and statuses.
pub mod sync;
/// Shared timeout parsing helpers for integration tests.
pub mod timeouts;
