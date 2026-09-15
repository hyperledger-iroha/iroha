//! Consume the actual source materializer's original entropy exactly once.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::ZkAmsPhase23MaterializedEncryptedSourceOwnerV1;

impl<R: crate::vega::MaskedRelaxedRandomSourceV1>
    GlobalLookupCommitmentSessionV1<R, SourceOpeningEntropyStageV1>
{
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn from_original_materialized_source_v1<
        K,
        P,
    >(
        owner: &mut ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<R, K, P>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        owner.validate_v1()?;
        if owner.original_random.is_none() || owner.bundle_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        // No bytes are requested while validating the source and reserving its
        // original inventory. The complete canonical source bundle binds this
        // session context; no caller supplies a replacement digest or RNG.
        let inventory = GlobalLookupCommitmentInventorySkeletonV1::new_v1()?;
        let original_random = owner
            .original_random
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        Ok(Self {
            live: Some(GlobalLookupCommitmentSessionLiveV1 {
                entropy: GlobalLookupProofSessionEntropySourceV1::Production {
                    original_random,
                    commitment_entropy_bytes: 0,
                },
                inventory,
                proof_session_context_digest: owner.bundle_digest,
                source_opening_context_digest: None,
                next_global_ordinal: 0,
                next_purpose: GlobalLookupCommitmentPurposeV1::Source,
                next_purpose_ordinal: 0,
                pending_source: None,
            }),
            state: PhantomData,
        })
    }
}

#[cfg(test)]
#[path = "original_entropy_handoff_v1_tests.rs"]
mod tests;
