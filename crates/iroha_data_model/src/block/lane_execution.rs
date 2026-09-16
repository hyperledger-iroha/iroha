//! Global economic transcripts for inputs certified by native lane instances.
//!
//! Native Decisions certify immutable input. These values describe the separate
//! deterministic global execution; neither decoding nor hashing grants live
//! authority, historical authority, or permission to acknowledge lane Apply.
//! TODO: replace the old merge execution DTO/consumer atomically with the native
//! runner and old-signer retirement; this format is not yet a production path.

use std::collections::BTreeSet;

use iroha_crypto::{Hash, HashOf, MerkleTree};
use norito::codec::{Decode, Encode};

use super::{BlockHeader, consensus::LaneBlockCommitment, lane_input::LaneDecisionGroupV1};
use crate::{
    fastpq::TransferTranscriptBundle,
    merge::MAX_MERGE_EXECUTION_BATCH_BYTES,
    nexus::MAX_ACTIVE_EXECUTION_LANES,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

const EXECUTION_DOMAIN: &[u8] = b"iroha:lane-consensus:economic-execution:v1\0";
const BATCH_DOMAIN: &[u8] = b"iroha:lane-consensus:economic-batch:v1\0";
const APPLICATION_DOMAIN: &[u8] = b"iroha:lane-consensus:economic-application:v1\0";

/// One admitted input, every route Decision, and its sole economic result.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_execution::LaneDecisionExecutionV1")]
pub struct LaneDecisionExecutionV1 {
    /// Exact input appears once, independently of the number of route Decisions.
    pub source: LaneDecisionGroupV1,
    /// Actual deterministic result at the enclosing global application context.
    pub result: TransactionResult,
    /// Inner signed identity authenticated from pre-carrier sealed-commitment state.
    /// A raw matching hash alone is not that stateful authentication.
    #[norito(required)]
    pub authenticated_signed_replay_alias: Option<Hash>,
    /// Actual economic fee/settlement evidence for the coordinator's route.
    pub settlement: LaneBlockCommitment,
    /// Executor-produced FASTPQ evidence for the single outer entrypoint.
    pub fastpq_transcripts: Vec<TransferTranscriptBundle>,
}

impl LaneDecisionExecutionV1 {
    /// Validate identities and cardinalities only, without authenticating execution.
    ///
    /// # Errors
    /// Rejects malformed sources, foreign settlement routes, duplicate transcript
    /// owners, and replay aliases that cannot name the exact sealed reveal.
    pub fn validate_structure(&self) -> Result<(), String> {
        self.source.validate_structure()?;
        let input = &self.source.payload.input;
        let coordinator = input.routing_plan()?.coordinator_route();
        let slot = self
            .source
            .payload
            .descriptor
            .slots
            .iter()
            .find(|slot| slot.route == coordinator)
            .ok_or_else(|| "native execution has no coordinator slot".to_owned())?;
        if (
            self.settlement.lane_id,
            self.settlement.dataspace_id,
            self.settlement.lane_incarnation,
            self.settlement.block_height,
        ) != (
            coordinator.lane_id,
            coordinator.dataspace_id,
            slot.lane_incarnation,
            slot.lane_height,
        ) || self.settlement.tx_count > 1
            || !self.settlement.native_amx_receipts.is_empty()
        {
            return Err("native execution settlement differs from its sole decided input".into());
        }
        if let Some(alias) = self.authenticated_signed_replay_alias {
            match &input.entrypoint {
                TransactionEntrypoint::SealedReveal(reveal)
                    if Hash::from(reveal.signed_transaction().hash()) == alias => {}
                _ => {
                    return Err(
                        "native execution replay alias is not its exact sealed signed identity"
                            .into(),
                    );
                }
            }
        }
        if self.fastpq_transcripts.len() > 1
            || self
                .fastpq_transcripts
                .iter()
                .any(|bundle| bundle.entry_hash != Hash::from(input.entrypoint.hash()))
        {
            return Err("native execution FASTPQ evidence has a foreign or duplicate owner".into());
        }
        Ok(())
    }

    /// Canonical domain-separated commitment, with no duplicated input or result hash.
    ///
    /// # Errors
    /// Rejects malformed shape or encoding failure; does not verify economic effects.
    pub fn canonical_hash(&self) -> Result<Hash, String> {
        self.validate_structure()?;
        let bytes = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        Ok(Hash::new_from_chunks(&[EXECUTION_DOMAIN, &bytes]))
    }
}

/// Exact global base and ordered economic transcript. Redundant counts, result
/// hashes and roots are derived from the sole sources instead of being encoded.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::block::lane_execution::LaneDecisionExecutionBatchV1")]
pub struct LaneDecisionExecutionBatchV1 {
    /// Canonical WSV base height, distinct from each route's input origin.
    pub base_state_height: u64,
    /// Exact canonical WSV snapshot identity, not a global block hash.
    pub base_state_hash: HashOf<BlockHeader>,
    /// Actual applying carrier's header with only payload/result roots stripped.
    pub application_block_header: BlockHeader,
    /// Strict first-admission order; each distinct route participates at most once.
    pub executions: Vec<LaneDecisionExecutionV1>,
    /// Ordered economic writes before self-identifying replay markers.
    pub application_write_set_root: Hash,
    /// Complete ordered writes including those deterministic replay markers.
    pub write_set_root: Hash,
}

impl LaneDecisionExecutionBatchV1 {
    /// Check bounded shape, canonical order and noncompeting route ownership.
    ///
    /// # Errors
    /// Rejects malformed headers, duplicates, conflicting routes or source shape.
    pub fn validate_structure(&self) -> Result<(), String> {
        let header = &self.application_block_header;
        if self.base_state_height == 0
            || self.base_state_height.checked_add(1) != Some(header.height().get())
            || header.prev_block_hash().is_none()
            || header.creation_time().is_zero()
            || header.merkle_root().is_some()
            || header.result_merkle_root().is_some()
            || *header != Self::application_header_from_carrier(header)
            || self.executions.is_empty()
            || self.executions.len() > MAX_ACTIVE_EXECUTION_LANES
        {
            return Err("native economic batch has an invalid carrier or group count".into());
        }
        let mut previous = None;
        let mut entrypoints = BTreeSet::new();
        let mut signed = BTreeSet::new();
        let mut commitments = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for execution in &self.executions {
            execution.validate_structure()?;
            let payload = &execution.source.payload;
            let priority = payload.descriptor.admission_priority;
            if priority.carrier_height > self.base_state_height
                || previous.is_some_and(|last| last >= priority)
                || !entrypoints.insert(payload.input.entrypoint.hash())
            {
                return Err("native economic batch repeats or reorders admitted work".into());
            }
            previous = Some(priority);
            let (signed_hash, commitment) = match &payload.input.entrypoint {
                TransactionEntrypoint::External(tx) => (Some(tx.hash()), None),
                TransactionEntrypoint::SealedReveal(reveal) => (
                    Some(reveal.signed_transaction().hash()),
                    Some(reveal.commitment),
                ),
                TransactionEntrypoint::SealedCommitment(commitment) => {
                    (None, Some(commitment.payload().commitment))
                }
                TransactionEntrypoint::Time(_) => {
                    return Err("native economic input is not a network entrypoint".into());
                }
            };
            if signed_hash.is_some_and(|hash| !signed.insert(hash))
                || commitment.is_some_and(|hash| !commitments.insert(hash))
                || payload
                    .descriptor
                    .slots
                    .iter()
                    .any(|slot| !routes.insert(slot.route_key()))
            {
                return Err(
                    "native economic batch repeats an execution identity or route slot".into(),
                );
            }
        }
        if routes.len() > MAX_ACTIVE_EXECUTION_LANES {
            return Err("native economic batch exceeds its total distinct route bound".into());
        }
        Ok(())
    }

    /// Strip payload-dependent commitments while preserving height, parent, time and view.
    pub fn application_header_from_carrier(carrier: &BlockHeader) -> BlockHeader {
        BlockHeader::new(
            carrier.height(),
            carrier.prev_block_hash(),
            None,
            None,
            u64::try_from(carrier.creation_time().as_millis()).unwrap_or(u64::MAX),
            carrier.view_change_index(),
        )
    }

    /// Derived entrypoint Merkle root in exact economic execution order.
    pub fn entrypoint_merkle_root(&self) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
        self.executions
            .iter()
            .map(|execution| execution.source.payload.input.entrypoint.hash())
            .collect::<MerkleTree<TransactionEntrypoint>>()
            .root()
    }

    /// Derived result Merkle root in the same order as the input root.
    pub fn result_merkle_root(&self) -> Option<HashOf<MerkleTree<TransactionResult>>> {
        self.executions
            .iter()
            .map(|execution| execution.result.hash())
            .collect::<MerkleTree<TransactionResult>>()
            .root()
    }

    /// Stable identity for replay markers, excluding only their dependent root.
    ///
    /// # Errors
    /// Rejects malformed shape or encoding failure. Marker-inclusive final batch
    /// hashing is separate, so inserting a marker creates no hash cycle.
    pub fn application_identity(&self) -> Result<Hash, String> {
        self.validate_structure()?;
        let mut pre_marker = self.clone();
        pre_marker.write_set_root = Hash::prehashed([0; Hash::LENGTH]);
        let bytes = norito::encode_canonical(&pre_marker).map_err(|error| error.to_string())?;
        Ok(Hash::new_from_chunks(&[APPLICATION_DOMAIN, &bytes]))
    }

    /// Final canonical commitment covers both pre-marker and complete write roots.
    ///
    /// # Errors
    /// Rejects malformed shape, encoding failure or the protocol batch byte cap.
    pub fn canonical_hash(&self) -> Result<Hash, String> {
        self.validate_structure()?;
        let bytes = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_MERGE_EXECUTION_BATCH_BYTES {
            return Err("native economic batch exceeds its protocol byte cap".into());
        }
        Ok(Hash::new_from_chunks(&[BATCH_DOMAIN, &bytes]))
    }

    /// Decode one canonical batch within both its protocol and actual carrier cap.
    ///
    /// # Errors
    /// Rejects empty/oversized/noncanonical frames or invalid structure. Native
    /// signatures and actual WSV replay must be verified by the consumer.
    pub fn decode_canonical(bytes: &[u8], carrier_cap: usize) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > carrier_cap.min(MAX_MERGE_EXECUTION_BATCH_BYTES) {
            return Err("native economic batch exceeds its enclosing carrier bound".into());
        }
        let frame = bytes.len();
        let allocation = norito::canonical_decode_limits(frame).max_total_allocated_bytes();
        let batch: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(frame, frame, frame, allocation, 64),
        )
        .map_err(|error| error.to_string())?;
        batch.validate_structure()?;
        Ok(batch)
    }
}
