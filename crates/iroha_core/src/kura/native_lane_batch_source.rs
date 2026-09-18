//! Canonical finalized inclusion of an inactive native input batch.
//! No live lane authority, execution validity or Apply receipt is granted here.

use super::Kura;
use crate::sumeragi::v2_transport::{
    AuthenticatedCertifiedBodyRequest, AuthenticatedCertifiedBodyResponse,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{
    BlockHeader, SignedBlock, consensus_v2::finality::V2FinalityArtifact,
    decode_framed_signed_block, lane_decision_batch::LaneDecisionBatchV1,
};
use std::num::NonZeroUsize;
type Result<T> = std::result::Result<T, String>;

/// Included source constructed only through actual canonical Kura finality.
#[derive(Clone, Debug)]
pub(crate) struct FinalizedNativeLaneBatchV1 {
    source: NativeLaneBatchRecoveryV1,
    carrier_header: BlockHeader,
    batch: LaneDecisionBatchV1,
}
impl FinalizedNativeLaneBatchV1 {
    /// Exact applying-carrier finality, not a caller's batch hash or roster.
    pub(crate) fn finality(&self) -> &V2FinalityArtifact {
        self.source.finality()
    }
    /// Exact globally authenticated applying header.
    pub(crate) fn carrier_header(&self) -> &BlockHeader {
        &self.carrier_header
    }
    /// Included inputs, still requiring actual pre-State authentication/replay.
    pub(crate) fn batch(&self) -> &LaneDecisionBatchV1 {
        &self.batch
    }
}

/// Named existing global-body recovery requirement minted by canonical Kura read.
#[derive(Clone, Debug)]
pub(crate) struct NativeLaneBatchRecoveryV1 {
    finality: V2FinalityArtifact,
}
impl NativeLaneBatchRecoveryV1 {
    /// Existing global request authority and signed RS16 geometry.
    pub(crate) fn finality(&self) -> &V2FinalityArtifact {
        &self.finality
    }
    /// Project an exact authenticated completion without acknowledging its owner.
    /// Resultless bytes never enter the executed-wire cache.
    pub(crate) fn complete_from_authenticated_response(
        &self,
        request: &AuthenticatedCertifiedBodyRequest,
        response: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<FinalizedNativeLaneBatchV1> {
        let request_wire = request.request();
        let response_wire = response.response();
        if request_wire.round != self.finality.commit_qc.proposal_round
            || request_wire.subject != self.finality.subject
            || request_wire.certificate != self.finality.commit_qc
            || response_wire.request_hash != request.request_hash()
            || Hash::new(&response_wire.body) != self.finality.subject.payload_hash
        {
            return Err("native batch recovery differs from its exact canonical request/QC".into());
        }
        let block = decode_framed_signed_block(&response_wire.body)
            .map_err(|error| format!("native batch recovery is not canonical: {error}"))?;
        if !block.is_resultless_proposal() {
            return Err("native batch recovery response contains execution results".into());
        }
        self.project(&block)
    }
    fn project(&self, block: &SignedBlock) -> Result<FinalizedNativeLaneBatchV1> {
        self.finality
            .validate_for_header(&block.header())
            .map_err(|error| error.to_string())?;
        if block.hash() != self.finality.block_hash
            || block
                .canonical_proposal_wire_hash()
                .map_err(|error| error.to_string())?
                != self.finality.subject.payload_hash
        {
            return Err("native batch body is not its exact finalized carrier".into());
        }
        if block.has_results() {
            let executed = block.encode_wire().map_err(|error| error.to_string())?;
            let commitment = &self.finality.commit_qc.execution_commitment;
            if u64::try_from(executed.len()).ok() != Some(commitment.executed_block_wire_len)
                || Hash::new(&executed) != commitment.executed_block_wire_hash
            {
                return Err("native batch executed image differs from finalized wire".into());
            }
        }
        // Retaining only header/batch must not erase extra controls and permit
        // an incomplete historical replay. Use the live scratch shape as well.
        let batch = crate::block::native_lane_batch_for_scratch(block)?;
        Ok(FinalizedNativeLaneBatchV1 {
            source: self.clone(),
            carrier_header: block.header(),
            batch: batch.clone(),
        })
    }
}

/// Storage corruption and authenticated body eviction are never conflated.
#[derive(Debug)]
pub(crate) enum NativeLaneBatchCarrierReadV1 {
    /// Immutable included input; actual pre-State replay remains required.
    Ready(FinalizedNativeLaneBatchV1),
    /// Install/retain the exact existing certified-body request before waiting.
    CanonicalBodyRecoveryRequired(NativeLaneBatchRecoveryV1),
}
impl Kura {
    /// Read exact canonical finality and body under the existing fallible kernel.
    /// Raw height/hash are locators only. Missing proof or malformed/replaced
    /// bytes are errors; authenticated body eviction yields a typed requirement.
    pub(crate) fn read_finalized_native_lane_batch(
        &self,
        height: NonZeroUsize,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<NativeLaneBatchCarrierReadV1> {
        let read = self
            .read_first_admission_carrier(height, expected_hash)
            .map_err(|error| error.to_string())?;
        let source = NativeLaneBatchRecoveryV1 {
            finality: read.finality,
        };
        match read.body {
            Some(body) => source
                .project(&body)
                .map(NativeLaneBatchCarrierReadV1::Ready),
            None => Ok(NativeLaneBatchCarrierReadV1::CanonicalBodyRecoveryRequired(
                source,
            )),
        }
    }
}
