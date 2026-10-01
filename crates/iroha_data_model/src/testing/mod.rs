//! Shared test fixtures for SDKs and guardrails.
//!
//! These helpers expose canonical wire fixtures used across guard scripts,
//! generators, and SDK regression tests.
/// Atomic cross-transaction fixtures.
pub mod axt;
/// Canonical V1 appeal-finance cancellation fixtures.
pub mod cancel_asset_lock;
/// Deterministic KAGEMUSHA V1 signing fixtures.
pub mod kagemusha;

/// Canonical threshold-signed Experimental release fixtures for operator and SDK tests.
pub mod kagemusha_release {
    pub use crate::kagemusha::kagemusha_release_v1::fixture_support::KagemushaExperimentalReleaseFixtureV1;
}

/// Genuine native certificate/checkpoint fixtures; execution outputs remain synthetic test inputs.
#[cfg(feature = "transparent_api")]
pub mod native_finality;

/// Genuine model crypto admission over explicit synthetic ordinary platform evidence.
pub mod ordinary_app_enrollment;
