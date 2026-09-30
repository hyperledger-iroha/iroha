//! Compact restart trust for an independently authenticated current-consensus prefix.
//!
//! Importing this DTO is an explicit trust-root operation. Its selected genesis and schedule
//! commitments must be authenticated outside the response being verified (for example, by an
//! operator-selected local checkpoint). A peer response must never become its own checkpoint.
use super::*;

/// Maximum canonical checkpoint: one signed genesis, one current proof and bounded committees.
pub const MAX_FINALITY_CHECKPOINT_BYTES: usize = 2 * MAX_FINALITY_BLOCK_BYTES + 4 * 1024 * 1024;

#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema, iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::CheckpointDecision")]
struct CheckpointDecision {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    core_hash: [u8; 32],
    result: [u8; 32],
    committee_digest: [u8; 32],
    schedule: ScheduleOutcome,
    beacon: Option<crate::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
    executed_hash: Hash,
    executed_len: u64,
}
impl CheckpointDecision {
    fn capture(height: u64, value: &Decision) -> Self {
        Self {
            height,
            block_hash: value.block_hash,
            core_hash: value.core_hash.0,
            result: value.result.0,
            committee_digest: value.committee_digest,
            schedule: value.schedule.clone(),
            beacon: value.beacon,
            executed_hash: value.executed_hash,
            executed_len: value.executed_len,
        }
    }
    fn decision(&self) -> Decision {
        Decision {
            block_hash: self.block_hash,
            core_hash: Hash32(self.core_hash),
            result: Hash32(self.result),
            committee_digest: self.committee_digest,
            schedule: self.schedule.clone(),
            beacon: self.beacon,
            executed_hash: self.executed_hash,
            executed_len: self.executed_len,
        }
    }
}

/// Canonical compact checkpoint exported from an authenticated prefix.
///
/// It retains the tip and at most two predecessor decisions needed for exact-tip and epoch-bound
/// successor verification. Private fields prevent accidental construction from an unverified
/// proof; decoding still yields an untrusted DTO. Independent local selection authenticates its
/// genesis, chain label and retained schedule commitments before `from_trusted_checkpoint`.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema, iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint")]
pub struct SumeragiFinalityCheckpoint {
    network_id: NetworkId,
    chain_id: String,
    genesis_wire: Vec<u8>,
    genesis_committee: Vec<FinalityValidator>,
    decisions: Vec<CheckpointDecision>,
    tip: SumeragiFinalityProof,
}
impl SumeragiFinalityCheckpoint {
    /// Network identity derived from the independently selected signed genesis.
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Independently selected current-consensus chain label.
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }
    /// Checkpoint height; import must authenticate this decoded claim before use.
    pub fn height(&self) -> u64 {
        self.tip.height()
    }
    /// Checkpoint block hash; import must authenticate this decoded claim before use.
    pub fn block_hash(&self) -> HashOf<BlockHeader> {
        self.tip.block_header.hash()
    }
    /// Retained proof of the checkpoint's exact certified decision.
    pub const fn tip(&self) -> &SumeragiFinalityProof {
        &self.tip
    }

    /// Encode this checkpoint in the sole canonical layout with finite resource bounds.
    ///
    /// # Errors
    /// Malformed bounds or encoding failure; this does not authenticate decoded trust roots.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, FinalityError> {
        self.validate_bounds()?;
        norito::encode_canonical(self).map_err(malformed)
    }
    /// Decode bounded canonical material; independent checkpoint selection is still required.
    ///
    /// # Errors
    /// Empty, oversized, noncanonical or structurally malformed checkpoint.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, FinalityError> {
        need(
            !bytes.is_empty() && bytes.len() <= MAX_FINALITY_CHECKPOINT_BYTES,
            "checkpoint frame exceeds bound",
        )?;
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(malformed)?;
        value.validate_bounds()?;
        Ok(value)
    }
    fn validate_bounds(&self) -> Result<(), FinalityError> {
        need(
            !self.chain_id.is_empty() && self.chain_id.len() <= 1024,
            "checkpoint chain label exceeds bound",
        )?;
        need(
            !self.genesis_wire.is_empty() && self.genesis_wire.len() <= MAX_FINALITY_BLOCK_BYTES,
            "checkpoint genesis exceeds bound",
        )?;
        need(
            !self.tip.block_wire.is_empty()
                && self.tip.block_wire.len() <= MAX_FINALITY_BLOCK_BYTES,
            "checkpoint tip exceeds bound",
        )?;
        need(
            !self.genesis_committee.is_empty()
                && self.genesis_committee.len() <= iroha_sumeragi::types::MAX_COMMITTEE_SIZE,
            "checkpoint genesis committee exceeds bound",
        )?;
        need(
            !self.tip.committee.is_empty()
                && self.tip.committee.len() <= iroha_sumeragi::types::MAX_COMMITTEE_SIZE,
            "checkpoint tip committee exceeds bound",
        )?;
        need(
            self.height() > 0
                && self.height() < u64::MAX
                && self.decisions.len() == self.height().min(3) as usize,
            "checkpoint requires an extensible tip and two predecessor commitments",
        )?;
        let first = self.height().saturating_sub(2).max(1);
        for (offset, decision) in self.decisions.iter().enumerate() {
            need(
                decision.height == first + offset as u64
                    && decision.core_hash != [0; 32]
                    && decision.result != [0; 32]
                    && decision.committee_digest != [0; 32]
                    && decision.schedule.height == decision.height
                    && decision.schedule.validate().is_ok()
                    && decision.executed_len > 0
                    && decision.executed_len <= MAX_FINALITY_BLOCK_BYTES as u64,
                "checkpoint commitments are malformed or discontinuous",
            )?;
        }
        need(
            norito::canonical_frame_len(self).map_err(malformed)? <= MAX_FINALITY_CHECKPOINT_BYTES,
            "checkpoint exceeds canonical byte bound",
        )
    }
}

impl SumeragiFinalityVerifier {
    /// Export only the tip of this already authenticated prefix and its bounded restart context.
    ///
    /// # Errors
    /// Missing prefix, a non-tip proof, substituted decision or invalid certificate.
    pub fn export_checkpoint(
        &self,
        tip: &SumeragiFinalityProof,
    ) -> Result<SumeragiFinalityCheckpoint, FinalityError> {
        need(
            self.decisions.last_key_value().map(|(height, _)| *height) == Some(tip.height()),
            "checkpoint must export the authenticated tip",
        )?;
        self.verify_same_decision(tip, tip)?;
        let first = tip.height().saturating_sub(2).max(1);
        let checkpoint = SumeragiFinalityCheckpoint {
            network_id: NetworkId::from_genesis_hash(self.genesis.hash()),
            chain_id: self.chain_id.clone(),
            genesis_wire: self
                .genesis
                .canonical_resultless_proposal()
                .map_err(malformed)?
                .encode_wire()
                .map_err(malformed)?,
            genesis_committee: self.genesis_committee.clone(),
            decisions: self
                .decisions
                .range(first..)
                .map(|(height, decision)| CheckpointDecision::capture(*height, decision))
                .collect(),
            tip: tip.clone(),
        };
        checkpoint.validate_bounds()?;
        Ok(checkpoint)
    }

    /// Import an independently authenticated local checkpoint as the new trust root.
    ///
    /// The caller must authenticate the selected file's genesis and retained schedule/result
    /// commitments independently of any new proof response. This checks internal consistency
    /// and re-verifies the tip certificate; it cannot establish the provenance of that selection.
    /// A remote checkpoint or one copied from the response is never an acceptable argument.
    ///
    /// # Errors
    /// Bounds, network/chain/genesis mismatch, discontinuous commitments, substituted tip,
    /// committee, invalid proof of possession or failed certificate authentication.
    pub fn from_trusted_checkpoint(
        checkpoint: &SumeragiFinalityCheckpoint,
        network: &NetworkId,
        chain_id: &str,
    ) -> Result<Self, FinalityError> {
        checkpoint.validate_bounds()?;
        need(
            checkpoint.network_id == *network && checkpoint.chain_id == chain_id,
            "checkpoint differs from independently selected network or chain",
        )?;
        let genesis = norito::core::with_decode_limits_scope(
            norito::canonical_decode_limits(checkpoint.genesis_wire.len()),
            || decode_versioned_signed_block(&checkpoint.genesis_wire),
        )
        .map_err(malformed)?;
        need(
            genesis.header().is_genesis()
                && genesis.hash().as_ref() == network.as_bytes()
                && genesis
                    .canonical_resultless_proposal()
                    .map_err(malformed)?
                    .encode_wire()
                    .map_err(malformed)?
                    == checkpoint.genesis_wire,
            "checkpoint genesis differs from selected network or canonical root",
        )?;
        let mut verifier = Self::new(&genesis, chain_id, checkpoint.genesis_committee.clone())?;
        verifier.decisions = checkpoint
            .decisions
            .iter()
            .map(|decision| (decision.height, decision.decision()))
            .collect();
        for decision in checkpoint
            .decisions
            .iter()
            .filter(|decision| decision.height <= 2)
        {
            need(
                decision.committee_digest == verifier.genesis_committee_digest,
                "checkpoint initial committee differs from genesis selection",
            )?;
            if decision.height == 1 {
                need(
                    decision.block_hash == genesis.hash()
                        && decision.core_hash == *genesis.hash().as_ref()
                        && decision.schedule.current == verifier.genesis_epoch,
                    "checkpoint genesis commitment differs from selected signed root",
                )?;
            }
        }
        verifier.verify_same_decision(&checkpoint.tip, &checkpoint.tip)?;
        Ok(verifier)
    }
}

#[cfg(all(test, feature = "transparent_api"))]
mod tests;
