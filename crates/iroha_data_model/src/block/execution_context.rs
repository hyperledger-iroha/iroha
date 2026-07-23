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
    peer::PeerId,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

/// Current wire version for a globally committed autonomous lane payload.
pub const AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1: u8 = 1;
/// Current-only first-release block execution-context bundle layout.
pub const BLOCK_EXECUTION_CONTEXT_BUNDLE_VERSION_V1: u8 = 1;

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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
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

/// Globally ordered commitment to one producer-authenticated autonomous lane payload.
///
/// `canonical_payload` contains the exact canonical framed bytes of the
/// hint-free lane payload. The duplicated identity fields let admission reject
/// substitutions before making the payload eligible for lane-local execution.
/// A finalized global block hint is attached only after this envelope has been
/// committed, so the payload never has to contain the hash of its own carrier.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct AutonomousLanePayloadEnvelopeV1 {
    /// Envelope schema version.
    pub version: u8,
    /// Hash of the chain identifier that owns the payload.
    pub chain_id_hash: Hash,
    /// Consensus epoch at the global proposal height.
    pub epoch: u64,
    /// Lane selected for autonomous execution.
    pub lane_id: LaneId,
    /// Dataspace selected for autonomous execution.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation.
    pub lane_incarnation: Hash,
    /// Global proposal height bound by the lane proposal.
    pub proposal_height: u64,
    /// Lane-local block height.
    pub lane_block_height: u64,
    /// Origin lane-local view.
    pub lane_block_view: u64,
    /// Exact producer-authenticated proposal identity.
    pub proposal_hash: Hash,
    /// Exact lane-block descriptor identity.
    pub descriptor_hash: Hash,
    /// Exact hint-neutral executable payload identity.
    pub payload_hash: Hash,
    /// Lane committee member that authenticated the payload.
    pub producer: PeerId,
    /// Bounded canonical framed bytes of the hint-free executable payload.
    pub canonical_payload: Vec<u8>,
}

/// Ordered execution context for external entrypoints in a block payload.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct BlockExecutionContextBundle {
    /// Exact first-release bundle layout. Only version one is supported.
    pub version: u8,
    /// Routing context entries aligned with the block's external entrypoints.
    pub external: Vec<ExternalExecutionContext>,
    /// Producer-authenticated autonomous payloads anchored by this global block.
    ///
    /// This field is deliberately required in the current layout. Legacy
    /// execution-context bytes must not decode as an implicitly empty anchor.
    pub autonomous_lane_payloads: Vec<AutonomousLanePayloadEnvelopeV1>,
    /// Lane-local payload ownership and RBC instance identities aligned by block entrypoint index.
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Merge-committee-certified entry applied before ordinary block entrypoints.
    pub merge_entry: Option<CertifiedMergeLedgerReference>,
}

impl BlockExecutionContextBundle {
    /// Current supported bundle layout.
    pub const VERSION: u8 = BLOCK_EXECUTION_CONTEXT_BUNDLE_VERSION_V1;

    /// Return whether this bundle advertises the current first-release layout.
    #[must_use]
    pub const fn has_current_version(&self) -> bool {
        self.version == Self::VERSION
    }

    /// Construct an ordered execution context bundle.
    #[must_use]
    pub const fn new(external: Vec<ExternalExecutionContext>) -> Self {
        Self {
            version: Self::VERSION,
            external,
            autonomous_lane_payloads: Vec::new(),
            lane_payload_ownerships: Vec::new(),
            merge_entry: None,
        }
    }

    /// Attach globally anchored autonomous lane payloads to this bundle.
    #[must_use]
    pub fn with_autonomous_lane_payloads(
        mut self,
        autonomous_lane_payloads: Vec<AutonomousLanePayloadEnvelopeV1>,
    ) -> Self {
        self.autonomous_lane_payloads = autonomous_lane_payloads;
        self
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
            && self.autonomous_lane_payloads.is_empty()
            && self.lane_payload_ownerships.is_empty()
            && self.merge_entry.is_none()
    }
}

impl Default for BlockExecutionContextBundle {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};

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
            version: MergeLedgerEntry::VERSION,
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
    fn block_execution_context_bundle_new_has_required_empty_autonomous_anchor() {
        let bundle = BlockExecutionContextBundle::new(Vec::new());
        assert_eq!(bundle.version, BlockExecutionContextBundle::VERSION);
        assert!(bundle.autonomous_lane_payloads.is_empty());
        assert!(bundle.is_empty());

        let encoded = norito::to_bytes(&bundle).expect("empty current bundle encodes");
        let decoded: BlockExecutionContextBundle =
            norito::decode_from_bytes(&encoded).expect("required empty anchor decodes");
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn block_execution_context_bundle_roundtrips_autonomous_lane_payloads() {
        let producer = PeerId::new(
            KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::BlsNormal)
                .expect("generate checked autonomous payload producer")
                .public_key()
                .clone(),
        );
        let envelope = AutonomousLanePayloadEnvelopeV1 {
            version: AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1,
            chain_id_hash: Hash::new(b"autonomous-envelope-chain"),
            epoch: 4,
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(9),
            lane_incarnation: Hash::new(b"autonomous-envelope-incarnation"),
            proposal_height: 7,
            lane_block_height: 5,
            lane_block_view: 0,
            proposal_hash: Hash::new(b"autonomous-envelope-proposal"),
            descriptor_hash: Hash::new(b"autonomous-envelope-descriptor"),
            payload_hash: Hash::new(b"autonomous-envelope-payload"),
            producer,
            canonical_payload: vec![1, 2, 3, 4],
        };
        let bundle = BlockExecutionContextBundle::new(Vec::new())
            .with_autonomous_lane_payloads(vec![envelope.clone()]);

        let encoded = norito::to_bytes(&bundle).expect("autonomous payload bundle encodes");
        let decoded: BlockExecutionContextBundle =
            norito::decode_from_bytes(&encoded).expect("autonomous payload bundle decodes");

        assert_eq!(decoded.autonomous_lane_payloads, vec![envelope]);
        assert!(!decoded.is_empty());
    }

    #[test]
    fn block_execution_context_layout_omissions_fail_closed() {
        #[derive(Encode)]
        struct LegacyExternalExecutionContext {
            entrypoint_hash: HashOf<TransactionEntrypoint>,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            routing_plan_digest: Hash,
            routing_plan_legs: Vec<ExternalExecutionRouteLeg>,
        }

        #[derive(Encode)]
        struct UnversionedBlockExecutionContextBundle {
            external: Vec<ExternalExecutionContext>,
            autonomous_lane_payloads: Vec<AutonomousLanePayloadEnvelopeV1>,
            lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
            merge_entry: Option<CertifiedMergeLedgerReference>,
        }

        #[derive(Encode)]
        struct PreAutonomousBlockExecutionContextBundle {
            version: u8,
            external: Vec<ExternalExecutionContext>,
            lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
            merge_entry: Option<CertifiedMergeLedgerReference>,
        }

        #[derive(Encode)]
        struct LegacyBlockExecutionContextBundle {
            version: u8,
            external: Vec<ExternalExecutionContext>,
            autonomous_lane_payloads: Vec<AutonomousLanePayloadEnvelopeV1>,
        }

        #[derive(Encode)]
        struct PreviousBlockExecutionContextBundle {
            version: u8,
            external: Vec<ExternalExecutionContext>,
            autonomous_lane_payloads: Vec<AutonomousLanePayloadEnvelopeV1>,
            lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
        }

        let legacy_external = LegacyExternalExecutionContext {
            entrypoint_hash: entrypoint_hash(b"legacy-external"),
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            routing_plan_digest: Hash::new(b"legacy-external-plan"),
            routing_plan_legs: Vec::new(),
        }
        .encode();
        assert!(
            ExternalExecutionContext::decode(&mut legacy_external.as_slice()).is_err(),
            "an external context omitting its Native AMX receipt field must fail closed"
        );

        let unversioned = UnversionedBlockExecutionContextBundle {
            external: Vec::new(),
            autonomous_lane_payloads: Vec::new(),
            lane_payload_ownerships: Vec::new(),
            merge_entry: None,
        }
        .encode();
        assert!(
            BlockExecutionContextBundle::decode(&mut unversioned.as_slice()).is_err(),
            "the unversioned execution-context bundle must fail closed"
        );

        let pre_autonomous = PreAutonomousBlockExecutionContextBundle {
            version: BlockExecutionContextBundle::VERSION,
            external: Vec::new(),
            lane_payload_ownerships: Vec::new(),
            merge_entry: None,
        }
        .encode();
        assert!(
            BlockExecutionContextBundle::decode(&mut pre_autonomous.as_slice()).is_err(),
            "the bundle layout omitting its autonomous payload field must fail closed"
        );

        let legacy = LegacyBlockExecutionContextBundle {
            version: BlockExecutionContextBundle::VERSION,
            external: Vec::new(),
            autonomous_lane_payloads: Vec::new(),
        }
        .encode();
        assert!(
            BlockExecutionContextBundle::decode(&mut legacy.as_slice()).is_err(),
            "the bundle layout omitting ownership and merge fields must fail closed"
        );

        let previous = PreviousBlockExecutionContextBundle {
            version: BlockExecutionContextBundle::VERSION,
            external: Vec::new(),
            autonomous_lane_payloads: Vec::new(),
            lane_payload_ownerships: Vec::new(),
        }
        .encode();
        assert!(
            BlockExecutionContextBundle::decode(&mut previous.as_slice()).is_err(),
            "the bundle layout omitting its merge field must fail closed"
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
