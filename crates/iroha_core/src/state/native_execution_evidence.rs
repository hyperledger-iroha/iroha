//! Bounded chronological authentication of global carriers and their complete lane state.
//!
//! The sole native certificate embedded in SignedBlockWire authenticates R and its mandatory
//! complete lane-state proof. No old finality artifact or separately encoded proof is accepted.
//! Genesis execution remains untrusted until its actual H2 successor is fully verified.

use super::NativeExecutionProjectionV1;
use crate::sumeragi::certified_chain::{CertifiedPrefix, CommittedBlock};
use iroha_data_model::{NetworkId, block::SignedBlock, sumeragi_lanes::SumeragiLaneState};
use iroha_model_base::chain::ChainId;
use std::{collections::BTreeMap, sync::Arc};

/// Finite byte/work bounds chosen independently of supplied evidence.
#[derive(Debug, Clone, Copy)]
pub struct NativeExecutionEvidenceLimits {
    /// Maximum contiguous carriers, including signed genesis and empty carriers.
    pub max_carriers: u64,
    /// Maximum complete SignedBlockWire bytes per carrier, including native certificate.
    pub max_carrier_bytes: u64,
    /// Maximum canonical complete context projection bytes per carrier.
    pub max_context_bytes: u64,
    /// Maximum cumulative retained complete carrier and context projection encodings.
    pub max_retained_bytes: u64,
}
struct RetainedCarrier {
    block: Arc<SignedBlock>,
}

/// Exact authenticated global carrier and its complete lane state for read-only queries.
/// This receipt does not grant a lane signing or execution capability.
#[derive(Debug)]
pub struct VerifiedNativeExecutionCarrier {
    block: Arc<SignedBlock>,
    lanes: Arc<SumeragiLaneState>,
    ordinary_writes_root: iroha_crypto::Hash,
    core_hash: iroha_sumeragi::types::Hash32,
    result: iroha_sumeragi::types::Hash32,
}
impl VerifiedNativeExecutionCarrier {
    /// Borrow the exact carrier authenticated by its native certificate.
    pub fn block(&self) -> &SignedBlock {
        &self.block
    }
    /// Original complete-write root from the same independently authenticated execution.
    pub(crate) fn ordinary_writes_root(&self) -> iroha_crypto::Hash {
        self.ordinary_writes_root
    }
    /// Match the complete original native execution cut without turning local certificate
    /// bytes into a new State tip capability.
    pub(crate) fn matches_original_tip(&self, tip: super::NativeExecutionTip) -> bool {
        self.block.header().height().get() == tip.height()
            && self.block.hash() == tip.iroha_hash()
            && self.core_hash == tip.core_hash()
            && self.result == tip.result()
    }
    /// Borrow the complete post-execution global lane state committed in this carrier's R.
    pub fn lanes(&self) -> &SumeragiLaneState {
        &self.lanes
    }
}

/// One consuming native evidence interval pinned independently to chain and genesis network.
/// Any error poisons the interval; no partial success can be promoted to another trust root.
pub struct NativeExecutionEvidenceVerifier {
    finality: Option<CertifiedPrefix>,
    chain_id: ChainId,
    network: NetworkId,
    limits: NativeExecutionEvidenceLimits,
    retained_bytes: u64,
    pending_genesis: Option<NativeExecutionProjectionV1>,
    carriers: BTreeMap<u64, RetainedCarrier>,
    lanes: Option<Arc<SumeragiLaneState>>,
    poisoned: bool,
}
impl NativeExecutionEvidenceVerifier {
    /// Start an interval which must include exact signed genesis and every successor.
    /// # Errors
    /// Rejects absent or unordered finite limits. No received committee or context is a root.
    pub fn new(
        chain_id: ChainId,
        network: NetworkId,
        limits: NativeExecutionEvidenceLimits,
    ) -> Result<Self, String> {
        if limits.max_carriers < 2
            || limits.max_carrier_bytes == 0
            || limits.max_context_bytes == 0
            || limits.max_carrier_bytes > limits.max_retained_bytes
            || limits.max_context_bytes > limits.max_retained_bytes
        {
            return Err(
                "native evidence limits must admit a finite genesis/successor interval".into(),
            );
        }
        Ok(Self {
            finality: None,
            chain_id,
            network,
            limits,
            retained_bytes: 0,
            pending_genesis: None,
            carriers: BTreeMap::new(),
            lanes: None,
            poisoned: false,
        })
    }

    /// Verify the next exact original carrier and complete context values.
    /// The signed genesis returns None; H2 authenticates its R before any lane state is trusted.
    /// # Errors
    /// Rejects skipped/replayed carriers, false state values, invalid
    /// native BLS/Pasta/RS16 certificates, missing contiguous history and bounds.
    pub fn push_height(
        &mut self,
        block: SignedBlock,
        context_evidence: &[u8],
    ) -> Result<Option<VerifiedNativeExecutionCarrier>, String> {
        self.push_shared_height(Arc::new(block), context_evidence)
    }

    /// Reuse the exact immutable Kura carrier for the live original-source reader.
    pub(crate) fn push_shared_height(
        &mut self,
        block: Arc<SignedBlock>,
        context_evidence: &[u8],
    ) -> Result<Option<VerifiedNativeExecutionCarrier>, String> {
        self.push_shared_height_with_genesis(block, context_evidence, |_| Ok(()))
    }

    /// Deliver the original genesis receipt only when its actual successor authenticates it.
    /// A rejected callback poisons this same interval; it cannot grant prefix completion.
    pub(crate) fn push_shared_height_with_genesis(
        &mut self,
        block: Arc<SignedBlock>,
        context_evidence: &[u8],
        genesis: impl FnOnce(VerifiedNativeExecutionCarrier) -> Result<(), String>,
    ) -> Result<Option<VerifiedNativeExecutionCarrier>, String> {
        if self.poisoned {
            return Err("native evidence interval is poisoned".into());
        }
        self.poisoned = true;
        let result = self.push_height_inner(block, context_evidence, genesis);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn push_height_inner(
        &mut self,
        block: Arc<SignedBlock>,
        context_evidence: &[u8],
        accept_genesis: impl FnOnce(VerifiedNativeExecutionCarrier) -> Result<(), String>,
    ) -> Result<Option<VerifiedNativeExecutionCarrier>, String> {
        let body_bytes = u64::try_from(
            norito::canonical_frame_len(block.as_ref()).map_err(|error| error.to_string())?,
        )
        .ok()
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or("native carrier byte length overflow")?;
        let context_bytes =
            u64::try_from(context_evidence.len()).map_err(|error| error.to_string())?;
        let retained = self
            .retained_bytes
            .checked_add(body_bytes)
            .and_then(|bytes| bytes.checked_add(context_bytes))
            .ok_or("native retained evidence length overflow")?;
        let count = self.carriers.len() as u64 + u64::from(self.pending_genesis.is_some());
        if count >= self.limits.max_carriers
            || body_bytes > self.limits.max_carrier_bytes
            || context_bytes == 0
            || context_bytes > self.limits.max_context_bytes
            || retained > self.limits.max_retained_bytes
        {
            return Err("native evidence exceeds its independently admitted interval".into());
        }
        block
            .validate_output_merkle_cache()
            .map_err(|error| error.to_string())?;
        let projection: NativeExecutionProjectionV1 = norito::decode_canonical_with_limits(
            context_evidence,
            norito::canonical_decode_limits(context_evidence.len()),
        )
        .map_err(|error| error.to_string())?;
        if projection.carrier_height != block.header().height().get()
            || projection.carrier_hash != block.hash()
        {
            return Err("native context projection changes its exact carrier identity".into());
        }
        let Some(finality) = self.finality.as_mut() else {
            self.finality = Some(
                CertifiedPrefix::new(&self.chain_id, self.network, block)
                    .map_err(|error| error.to_string())?,
            );
            self.pending_genesis = Some(projection);
            self.retained_bytes = retained;
            return Ok(None);
        };
        let (verified, genesis) = finality
            .push(block)
            .map_err(|error| error.to_string())?
            .into_parts();
        if let Some(genesis) = genesis {
            let projection = self
                .pending_genesis
                .take()
                .ok_or("native genesis projection is missing")?;
            accept_genesis(self.accept_verified(genesis.into_committed(), projection)?)?;
        } else if self.pending_genesis.is_some() {
            return Err("native successor did not authenticate the original genesis result".into());
        }
        let receipt = self.accept_verified(verified.into_committed(), projection)?;
        self.retained_bytes = retained;
        Ok(Some(receipt))
    }

    /// Borrow a retained original carrier only after the interval has its actual H2 anchor.
    pub(crate) fn authenticated_carrier(&self, height: u64) -> Option<Arc<SignedBlock>> {
        if self.poisoned || self.pending_genesis.is_some() || self.carriers.len() < 2 {
            return None;
        }
        self.carriers
            .get(&height)
            .map(|carrier| Arc::clone(&carrier.block))
    }

    /// Transfer the exact latest global lane state after a clean, fully anchored interval.
    pub(super) fn into_current_lanes(self) -> Result<Arc<SumeragiLaneState>, String> {
        if self.poisoned || self.pending_genesis.is_some() || self.carriers.len() < 2 {
            return Err("native lane state interval has not completed its successor anchor".into());
        }
        self.lanes
            .ok_or_else(|| "native lane state is missing".into())
    }

    fn accept_verified(
        &mut self,
        committed: CommittedBlock,
        evidence: NativeExecutionProjectionV1,
    ) -> Result<VerifiedNativeExecutionCarrier, String> {
        let block = Arc::clone(committed.block());
        let height = committed.height();
        if !committed.commitment().native_lanes.matches_state(
            self.network,
            height,
            &evidence.lanes,
        )? {
            return Err("complete lane state differs from the proof in certified R".into());
        }
        super::native_lane_state::validate_sumeragi_lane_state(
            self.network,
            height,
            &evidence.lanes,
        )?;
        // Lane execution lives in the sole LaneRunner/global merge path. This reader validates
        // the global certificate and complete state commitment; it creates no second executor.
        let lanes = Arc::new(evidence.lanes);
        self.carriers.insert(
            height,
            RetainedCarrier {
                block: Arc::clone(&block),
            },
        );
        self.lanes = Some(Arc::clone(&lanes));
        Ok(VerifiedNativeExecutionCarrier {
            block,
            lanes,
            ordinary_writes_root: committed.commitment().execution.ordinary_writes_root,
            core_hash: committed.core_hash(),
            result: committed.result(),
        })
    }
}

#[cfg(test)]
mod tests;
