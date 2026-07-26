//! Shared test fixtures for SDKs and guardrails.
//!
//! These helpers expose canonical wire fixtures used across guard scripts,
//! generators, and SDK regression tests.

/// Atomic cross-transaction fixtures.
pub mod axt;
/// Canonical V1 appeal-finance cancellation fixtures.
#[cfg(feature = "json")]
pub mod cancel_asset_lock;
