//! Canonical proof construction for public deterministic diagnostic fixtures.
//!
//! This test-only helper uses the same q77 producer, clearing source owner and
//! self-check as ordinary artifacts. Fixed fixture entropy is never a production
//! RNG. No alternate proof layout, transcript or verifier is implemented here.

use rand::{SeedableRng, rngs::StdRng};

use super::{
    deep_proof,
    deep_prover::{ConstructionLimits, ProducerPlan},
    deep_relation::DeepRelation,
    deep_trace_source::OwnedTraceSource,
    offline_compact::ProvingLimits,
};
use crate::{Result, VerifyLimits};

/// Prove one complete public fixture using explicit deterministic test entropy.
pub(super) fn prove(
    relation: &impl DeepRelation,
    source: OwnedTraceSource,
    seed: u64,
) -> Result<Vec<u8>> {
    let limits = ProvingLimits::default();
    ProducerPlan::new(
        relation,
        ConstructionLimits {
            digest_execution: limits.digest_execution,
            max_payload_bytes: limits.max_segment_charge_bytes,
            max_work_units: limits.max_segment_work_units,
            max_hash_calls: limits.max_segment_work_units,
            max_proof_bytes: deep_proof::PROOF_BYTE_TARGET,
        },
    )?
    .build(source, &mut StdRng::seed_from_u64(seed))
}

/// Exact admitted child-frame and query geometry within the unchanged ceilings.
pub(super) fn verification_limits() -> VerifyLimits {
    VerifyLimits {
        max_proof_bytes: deep_proof::MAX_FRAME_BYTES,
        max_queries: super::deep_geometry::QUERY_COUNT,
        ..VerifyLimits::default()
    }
}

#[cfg(test)]
#[path = "deep_fixture/retirement_tests.rs"]
mod retirement_tests;
