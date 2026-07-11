//! Durable execution routing context committed by a block header.

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    block::{
        BlockHeader,
        consensus::{NativeAmxReceipt, SumeragiLanePayloadOwnership},
    },
    merge::{MergeLedgerEntry, MergeQuorumCertificate},
    nexus::{DataSpaceId, LaneId},
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

/// Role of one route leg in an external execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(tag = "role", content = "detail", rename_all = "snake_case")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub enum ExternalExecutionRouteRole {
    /// The route coordinates final admission and commit ordering for the plan.
    Coordinator,
    /// The route prepares or commits one dataspace-local leg of the plan.
    Participant,
}

/// Lane/dataspace leg committed as part of an external execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ExternalExecutionRouteLeg {
    /// Lane selected for this leg.
    pub lane_id: LaneId,
    /// Dataspace selected for this leg.
    pub dataspace_id: DataSpaceId,
    /// Role assigned to this leg.
    pub role: ExternalExecutionRouteRole,
}

impl ExternalExecutionRouteLeg {
    /// Construct an execution route leg.
    #[must_use]
    pub const fn new(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        role: ExternalExecutionRouteRole,
    ) -> Self {
        Self {
            lane_id,
            dataspace_id,
            role,
        }
    }
}

/// Routing context used to execute one external block entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ExternalExecutionContext {
    /// Hash of the external entrypoint this context belongs to.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Lane selected for execution.
    pub lane_id: LaneId,
    /// Dataspace selected for execution.
    pub dataspace_id: DataSpaceId,
    /// Deterministic digest of the full routing plan used for execution.
    pub routing_plan_digest: Hash,
    /// Full coordinator/participant route plan used for execution.
    pub routing_plan_legs: Vec<ExternalExecutionRouteLeg>,
    /// Native AMX receipt collected for this routed entrypoint, when the plan spans dataspaces.
    #[norito(default)]
    pub native_amx_receipt: Option<NativeAmxReceipt>,
}

impl ExternalExecutionContext {
    /// Construct routing context for one external entrypoint.
    #[must_use]
    pub fn new(
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> Self {
        let routing_plan_legs = vec![ExternalExecutionRouteLeg::new(
            lane_id,
            dataspace_id,
            ExternalExecutionRouteRole::Coordinator,
        )];
        let routing_plan_digest = single_route_plan_digest(lane_id, dataspace_id);
        Self {
            entrypoint_hash,
            lane_id,
            dataspace_id,
            routing_plan_digest,
            routing_plan_legs,
            native_amx_receipt: None,
        }
    }

    /// Construct routing context with a committed full routing plan.
    #[must_use]
    pub fn with_routing_plan(
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        routing_plan_digest: Hash,
        routing_plan_legs: Vec<ExternalExecutionRouteLeg>,
    ) -> Self {
        Self {
            entrypoint_hash,
            lane_id,
            dataspace_id,
            routing_plan_digest,
            routing_plan_legs,
            native_amx_receipt: None,
        }
    }

    /// Attach a native AMX receipt to this execution context.
    #[must_use]
    pub fn with_native_amx_receipt(mut self, receipt: NativeAmxReceipt) -> Self {
        self.native_amx_receipt = Some(receipt);
        self
    }
}

fn single_route_plan_digest(lane_id: LaneId, dataspace_id: DataSpaceId) -> Hash {
    let mut bytes = Vec::with_capacity(16 + 12);
    bytes.extend_from_slice(b"iroha:routing-plan:v1");
    bytes.extend_from_slice(&lane_id.as_u32().to_le_bytes());
    bytes.extend_from_slice(&dataspace_id.as_u64().to_le_bytes());
    Hash::new(bytes)
}

/// Compact, globally ordered reference to a merge-committee-certified entry.
///
/// The complete merge entry can contain many lane payloads and is transferred as
/// a hash-addressed sidecar. Committed blocks carry this bounded reference so
/// merge execution shares the same total order as ordinary block transactions
/// without duplicating a potentially multi-megabyte transcript in consensus
/// frames.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CertifiedMergeLedgerReference {
    /// Reference schema version. Version one is the only valid value.
    pub version: u8,
    /// Canonical hash of the complete certified [`MergeLedgerEntry`].
    pub entry_hash: HashOf<MergeLedgerEntry>,
    /// Canonical encoded length of the complete entry, used to bound sidecar fetches.
    pub encoded_len: u64,
    /// Merge-ledger epoch sealed by the certificate.
    pub epoch_id: u64,
    /// Execution-batch hash when this entry applies autonomous lane work.
    pub execution_batch_hash: Option<Hash>,
    /// Number of entrypoint/result leaves in the certified execution batch.
    pub entrypoint_count: Option<u64>,
    /// Merkle root of the ordered certified entrypoint hashes.
    pub entrypoint_merkle_root: Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
    /// Merkle root of the ordered certified execution-result hashes.
    pub result_merkle_root: Option<HashOf<MerkleTree<TransactionResult>>>,
    /// Committed WSV height from which the execution batch starts.
    pub base_state_height: Option<u64>,
    /// Committed WSV identity from which the execution batch starts.
    pub base_state_hash: Option<HashOf<BlockHeader>>,
    /// Self-contained merge certificate; its signer bitmap also identifies sidecar holders.
    pub merge_qc: MergeQuorumCertificate,
}

impl CertifiedMergeLedgerReference {
    /// Construct the canonical compact reference for a complete merge entry.
    #[must_use]
    pub fn new(entry: &MergeLedgerEntry) -> Self {
        let (
            execution_batch_hash,
            entrypoint_count,
            entrypoint_merkle_root,
            result_merkle_root,
            base_state_height,
            base_state_hash,
        ) = entry
            .execution_batch
            .as_ref()
            .map_or((None, None, None, None, None, None), |batch| {
                (
                    Some(batch.batch_hash),
                    Some(batch.entrypoint_count),
                    Some(batch.entrypoint_merkle_root),
                    Some(batch.result_merkle_root),
                    Some(batch.base_state_height),
                    Some(batch.base_state_hash),
                )
            });
        Self {
            version: 1,
            entry_hash: entry.canonical_hash(),
            encoded_len: entry.canonical_encoded_len(),
            epoch_id: entry.epoch_id,
            execution_batch_hash,
            entrypoint_count,
            entrypoint_merkle_root,
            result_merkle_root,
            base_state_height,
            base_state_hash,
            merge_qc: entry.merge_qc.clone(),
        }
    }

    /// Return whether this reference exactly identifies `entry`.
    #[must_use]
    pub fn matches_entry(&self, entry: &MergeLedgerEntry) -> bool {
        self == &Self::new(entry)
    }
}

/// Ordered execution context for external entrypoints in a block payload.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockExecutionContextBundle {
    /// Routing context entries aligned with the block's external entrypoints.
    pub external: Vec<ExternalExecutionContext>,
    /// Lane-local payload ownership and RBC instance identities aligned by block entrypoint index.
    #[norito(default)]
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Merge-committee-certified entry applied before ordinary block entrypoints.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub merge_entry: Option<CertifiedMergeLedgerReference>,
}

impl BlockExecutionContextBundle {
    /// Construct an ordered execution context bundle.
    #[must_use]
    pub const fn new(external: Vec<ExternalExecutionContext>) -> Self {
        Self {
            external,
            lane_payload_ownerships: Vec::new(),
            merge_entry: None,
        }
    }

    /// Attach lane-local payload ownership identities to this bundle.
    #[must_use]
    pub fn with_lane_payload_ownerships(
        mut self,
        lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    ) -> Self {
        self.lane_payload_ownerships = lane_payload_ownerships;
        self
    }

    /// Attach a merge-ledger entry reference to this bundle.
    #[must_use]
    pub fn with_merge_entry(mut self, merge_entry: CertifiedMergeLedgerReference) -> Self {
        self.merge_entry = Some(merge_entry);
        self
    }

    /// Returns true when the bundle carries no execution context.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.external.is_empty()
            && self.lane_payload_ownerships.is_empty()
            && self.merge_entry.is_none()
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use super::*;
    use crate::{
        merge::{MergeExecutionBatch, MergeLedgerEntry},
        peer::PeerId,
    };

    fn entrypoint_hash(label: &[u8]) -> HashOf<TransactionEntrypoint> {
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(label))
    }

    fn sample_merge_entry() -> MergeLedgerEntry {
        let validator_set = Vec::<PeerId>::new();
        MergeLedgerEntry {
            epoch_id: 9,
            lane_catalog_hash: Hash::new(b"lane-catalog"),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new(b"incarnations"),
            activation_root: Hash::new(b"activations"),
            lane_snapshots: Vec::new(),
            global_state_root: Hash::new(b"global-state"),
            merge_qc: MergeQuorumCertificate::new(
                2,
                9,
                10,
                HashOf::from_untyped_unchecked(Hash::new(b"carrier-parent")),
                Hash::new(b"chain"),
                1,
                HashOf::new(&validator_set),
                validator_set,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"merge-message"),
            ),
            execution_batch: None,
            lane_drain_certificates: Vec::new(),
        }
    }

    fn sample_execution_batch() -> MergeExecutionBatch {
        let base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"batch-base-state"));
        MergeExecutionBatch {
            version: 1,
            base_state_height: 1,
            base_state_hash,
            application_block_header: BlockHeader::new(
                NonZeroU64::new(2).expect("non-zero block height"),
                Some(base_state_hash),
                None,
                None,
                7,
                0,
            ),
            lanes: Vec::new(),
            entrypoint_count: 3,
            entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"entrypoint-root")),
            result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"result-root")),
            execution_root: Hash::new(b"execution-root"),
            application_write_set_root: Hash::new(b"application-write-set"),
            write_set_root: Hash::new(b"write-set"),
            expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"post-state")),
            batch_hash: Hash::new(b"batch-hash"),
        }
    }

    #[test]
    fn external_execution_context_new_commits_single_route_plan() {
        let lane_id = LaneId::new(3);
        let dataspace_id = DataSpaceId::new(7);
        let context =
            ExternalExecutionContext::new(entrypoint_hash(b"entrypoint"), lane_id, dataspace_id);

        assert_eq!(context.lane_id, lane_id);
        assert_eq!(context.dataspace_id, dataspace_id);
        assert_eq!(
            context.routing_plan_digest,
            single_route_plan_digest(lane_id, dataspace_id)
        );
        assert_eq!(
            context.routing_plan_legs,
            vec![ExternalExecutionRouteLeg::new(
                lane_id,
                dataspace_id,
                ExternalExecutionRouteRole::Coordinator,
            )]
        );
    }

    #[test]
    fn external_execution_context_with_routing_plan_preserves_full_plan() {
        let lane_id = LaneId::new(1);
        let dataspace_id = DataSpaceId::new(7);
        let routing_plan_digest = Hash::new(b"native-amx-plan");
        let routing_plan_legs = vec![
            ExternalExecutionRouteLeg::new(
                lane_id,
                dataspace_id,
                ExternalExecutionRouteRole::Coordinator,
            ),
            ExternalExecutionRouteLeg::new(
                LaneId::new(2),
                DataSpaceId::new(8),
                ExternalExecutionRouteRole::Participant,
            ),
        ];

        let context = ExternalExecutionContext::with_routing_plan(
            entrypoint_hash(b"native-entrypoint"),
            lane_id,
            dataspace_id,
            routing_plan_digest,
            routing_plan_legs.clone(),
        );

        assert_eq!(context.routing_plan_digest, routing_plan_digest);
        assert_eq!(context.routing_plan_legs, routing_plan_legs);
    }

    #[test]
    fn block_execution_context_bundle_roundtrips_lane_payload_ownerships() {
        let lane_id = LaneId::new(2);
        let dataspace_id = DataSpaceId::new(9);
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 7,
            proposal_view: 3,
            lane_id,
            dataspace_id,
            lane_incarnation: Hash::new(b"execution-context-lane-incarnation"),
            lane_block_height: 5,
            lane_block_view: 1,
            subject_hash: Hash::new(b"lane-subject"),
            qc_mode_tag: "test-lane-qc-mode".to_string(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"entrypoint")],
            previous_lane_block_height: 4,
            previous_lane_block_descriptor_hash: Some(Hash::new(b"previous-lane-block-descriptor")),
            lane_block_descriptor_hash: Some(Hash::new(b"lane-block-descriptor")),
            lane_block_descriptor_validator_set: Vec::new(),
            lane_block_descriptor_validator_count: 0,
            lane_block_descriptor_min_quorum: 0,
            payload_ownership_hash: Hash::new(b"lane-payload-ownership"),
            rbc_instance_hash: Hash::new(b"lane-rbc-instance"),
        };
        let bundle = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            entrypoint_hash(b"entrypoint"),
            lane_id,
            dataspace_id,
        )])
        .with_lane_payload_ownerships(vec![ownership.clone()]);

        let encoded = norito::to_bytes(&bundle).expect("bundle encodes");
        let decoded: BlockExecutionContextBundle =
            norito::decode_from_bytes(&encoded).expect("bundle decodes with lane ownership");

        assert_eq!(decoded.lane_payload_ownerships, vec![ownership]);
        assert!(!decoded.is_empty());
    }

    #[test]
    fn block_execution_context_bundle_with_only_ownership_is_not_empty() {
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 1,
            proposal_view: 0,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation: Hash::new(b"execution-context-single-lane-incarnation"),
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"subject"),
            qc_mode_tag: "test-lane-qc-mode".to_string(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"entrypoint")],
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(Hash::new(b"descriptor")),
            lane_block_descriptor_validator_set: Vec::new(),
            lane_block_descriptor_validator_count: 0,
            lane_block_descriptor_min_quorum: 0,
            payload_ownership_hash: Hash::new(b"payload"),
            rbc_instance_hash: Hash::new(b"rbc"),
        };

        assert!(
            !BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership])
                .is_empty()
        );
    }

    #[test]
    fn certified_merge_reference_roundtrips_and_rejects_tampering() {
        let entry = sample_merge_entry();
        let reference = CertifiedMergeLedgerReference::new(&entry);
        assert!(reference.matches_entry(&entry));
        assert_eq!(reference.entrypoint_count, None);
        assert_eq!(reference.entrypoint_merkle_root, None);
        assert_eq!(reference.result_merkle_root, None);

        let bundle =
            BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(reference.clone());
        let encoded = norito::to_bytes(&bundle).expect("merge reference bundle encodes");
        let decoded: BlockExecutionContextBundle =
            norito::decode_from_bytes(&encoded).expect("merge reference bundle decodes");
        assert_eq!(decoded.merge_entry.as_ref(), Some(&reference));
        assert!(!decoded.is_empty());

        let mut tampered = reference;
        tampered.encoded_len = tampered.encoded_len.saturating_add(1);
        assert!(!tampered.matches_entry(&entry));

        let mut partial_batch_binding = CertifiedMergeLedgerReference::new(&entry);
        partial_batch_binding.entrypoint_count = Some(1);
        assert!(!partial_batch_binding.matches_entry(&entry));
    }

    #[test]
    fn certified_merge_reference_binds_transaction_proof_roots() {
        let mut entry = sample_merge_entry();
        let batch = sample_execution_batch();
        entry.execution_batch = Some(batch.clone());

        let reference = CertifiedMergeLedgerReference::new(&entry);
        assert_eq!(reference.execution_batch_hash, Some(batch.batch_hash));
        assert_eq!(reference.entrypoint_count, Some(batch.entrypoint_count));
        assert_eq!(
            reference.entrypoint_merkle_root,
            Some(batch.entrypoint_merkle_root)
        );
        assert_eq!(reference.result_merkle_root, Some(batch.result_merkle_root));
        assert!(reference.matches_entry(&entry));

        let mut tampered = reference;
        tampered.result_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"tampered-result-root",
        )));
        assert!(!tampered.matches_entry(&entry));
    }
}
