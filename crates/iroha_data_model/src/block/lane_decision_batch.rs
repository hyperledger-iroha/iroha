//! Ordered native Decision sources and their exact applying pre-State.
//!
//! Decisions certify immutable input. Economic results, settlement, FASTPQ
//! outputs and execution write roots are absent from proposals and are computed
//! using the actual enclosing carrier header. The global execution commitment
//! authenticates the sole complete result. Decoding these sources grants no
//! current-State, historical, native quorum or global publication authority.
//! TODO: activate only with the sole canonical consumer and old-signer retirement.

use std::collections::BTreeSet;

use iroha_crypto::{Hash, HashOf, MerkleTree};
use norito::codec::{Decode, Encode};

use super::{BlockHeader, lane_input::LaneDecisionGroupV1};
use crate::{
    merge::MAX_MERGE_EXECUTION_BATCH_BYTES, nexus::MAX_ACTIVE_EXECUTION_LANES,
    transaction::signed::TransactionEntrypoint,
};

const BATCH_DOMAIN: &[u8] = b"iroha:lane-consensus:decision-batch:v1\0";

/// Exact applying WSV base and canonically ordered immutable native input groups.
/// Counts and source roots are derived; no economic output belongs in this value.
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
#[norito_schema(name = "iroha_data_model::block::lane_decision_batch::LaneDecisionBatchV1")]
pub struct LaneDecisionBatchV1 {
    /// Canonical WSV base height, distinct from each input's first-admission height.
    pub base_state_height: u64,
    /// Exact canonical WSV snapshot identity, not a global block hash.
    pub base_state_hash: HashOf<BlockHeader>,
    /// Strict first-admission order; each distinct route participates at most once.
    pub groups: Vec<LaneDecisionGroupV1>,
}

impl LaneDecisionBatchV1 {
    /// Check bounded source shape, canonical priority and unique execution/route owners.
    ///
    /// # Errors
    /// Rejects zero base, empty/oversized groups, future/reordered admission ranks,
    /// repeated outer/signed/sealed identities, competing routes or malformed source.
    /// Exact applying header/base and cryptographic authority require the consumer.
    pub fn validate_structure(&self) -> Result<(), String> {
        if self.base_state_height == 0
            || self.base_state_height.checked_add(1).is_none()
            || self.groups.is_empty()
            || self.groups.len() > MAX_ACTIVE_EXECUTION_LANES
        {
            return Err("native decision batch has an invalid base or group count".into());
        }
        let mut previous = None;
        let mut entrypoints = BTreeSet::new();
        let mut signed = BTreeSet::new();
        let mut commitments = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for group in &self.groups {
            group.validate_structure()?;
            let payload = &group.payload;
            let priority = payload.descriptor.admission_priority;
            if priority.carrier_height > self.base_state_height
                || previous.is_some_and(|last| last >= priority)
                || !entrypoints.insert(payload.input.entrypoint.hash())
            {
                return Err("native decision batch repeats or reorders admitted work".into());
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
                    return Err("native input is not a network entrypoint".into());
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
                    "native decision batch repeats an execution identity or route slot".into(),
                );
            }
        }
        if routes.len() > MAX_ACTIVE_EXECUTION_LANES {
            return Err("native decision batch exceeds its total distinct route bound".into());
        }
        Ok(())
    }

    /// Derived input root in canonical source order; never a second header input root.
    pub fn entrypoint_merkle_root(&self) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
        self.groups
            .iter()
            .map(|group| group.payload.input.entrypoint.hash())
            .collect::<MerkleTree<TransactionEntrypoint>>()
            .root()
    }

    /// Canonical source commitment, independent of economic output and its write roots.
    ///
    /// # Errors
    /// Rejects malformed shape, encoding failure or the shared global batch byte cap.
    pub fn canonical_hash(&self) -> Result<Hash, String> {
        self.validate_structure()?;
        let bytes = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_MERGE_EXECUTION_BATCH_BYTES {
            return Err("native decision batch exceeds its protocol byte cap".into());
        }
        Ok(Hash::new_from_chunks(&[BATCH_DOMAIN, &bytes]))
    }

    /// Decode one canonical source batch within its protocol and enclosing carrier caps.
    ///
    /// # Errors
    /// Rejects empty, oversized, noncanonical or malformed sources. Native signatures,
    /// actual first-admission inclusion and exact applying pre-State remain unauthenticated.
    pub fn decode_canonical(bytes: &[u8], carrier_cap: usize) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > carrier_cap.min(MAX_MERGE_EXECUTION_BATCH_BYTES) {
            return Err("native decision batch exceeds its enclosing carrier bound".into());
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
