#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test harness covering cross-cutting Iroha scenarios.

/// Focused genesis encoding diagnostics.
#[path = "debug_genesis.rs"]
mod debug_genesis;
/// Event stream and notification integration tests.
mod events;
/// Prolonged and extra-functional behaviour checks (e.g., network churn).
mod extra_functional;
/// Query API regression coverage.
mod nexus;
mod queries;
/// Data availability and RBC integration coverage.
mod sumeragi_da;
/// NPoS-specific Sumeragi behaviour checks.
mod sumeragi_npos_liveness;
/// Trigger lifecycle and execution paths.
mod triggers;
