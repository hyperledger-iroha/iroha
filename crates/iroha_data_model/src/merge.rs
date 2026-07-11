//! Merge-ledger data structures.
//!
//! The merge ledger records a compact ordered log of lane tips along with the
//! deterministic global reduction state used to finalize world state updates.
//! These DTOs provide the on-wire and persistence representations of merge
//! entries. See `docs/source/merge_ledger.md` for the normative behaviour the
//! runtime must enforce when producing and validating these records.

use iroha_crypto::{Hash, HashOf, MerkleTree, PublicKey};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    block::{
        BlockHeader,
        consensus::{
            LaneBlockCommitment, LaneBlockProposalV1, LaneBlockQcV1, NativeAmxReceipt,
            ValidatorIndex,
        },
    },
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

const MERGE_LEDGER_ENTRY_HASH_DOMAIN: &[u8] = b"iroha:merge:ledger-entry:v1\0";

/// Maximum canonical framed size of one full merge-ledger entry.
///
/// This is the protocol-wide limit used by pending sidecars, compact block
/// references, recovery, and admission. Keep all consumers on this constant so
/// a certified entry cannot be accepted by one layer and rejected by another.
pub const MAX_MERGE_LEDGER_ENTRY_BYTES: usize = 16 * 1024 * 1024;

/// Maximum canonical size of the execution-batch field inside a merge entry.
///
/// Four MiB of the full-entry envelope is reserved for the active-lane set,
/// snapshots, merge QC, and Norito framing. Admission still checks the exact
/// final full-entry size after the QC is attached.
pub const MAX_MERGE_EXECUTION_BATCH_BYTES: usize = 12 * 1024 * 1024;

/// Maximum number of ordered entrypoints in one certified merge execution batch.
///
/// Lane-local proposal admission uses the same ceiling so every certified lane
/// block is individually eligible for inclusion in a global merge batch.
pub const MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096;

/// Maximum canonical size of one authenticated autonomous-lane source bundle.
///
/// A merge execution repeats selected payload material beside this bundle and
/// adds deterministic results, settlement evidence, and committee proofs. The
/// four-MiB source ceiling therefore preserves enough of the 12-MiB execution
/// envelope for every individually certified lane source to remain mergeable.
pub const MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES: usize = 4 * 1024 * 1024;

/// Maximum canonical size reserved for the certified-proposal half of one source bundle.
///
/// The certified envelope repeats the bounded entrypoint commitments in the proposal and
/// prepare/commit vote bodies and carries the validator sets, availability QC, and signer PoPs.
/// Keeping this reservation protocol-wide prevents an autonomous view chain from consuming the
/// complete source budget before the globally executable certificate is attached.
pub const MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES: usize = 1024 * 1024;

/// Maximum canonical size of the autonomous payload and authenticated view-chain half.
///
/// NewView persistence checkpoints before this budget is exceeded. The independently bounded
/// certified envelope and the exact final bundle check then keep every accepted source within
/// [`MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES`].
pub const MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES: usize =
    MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES - MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES;

/// Return whether an exact canonical full-entry length is protocol-admissible.
#[must_use]
pub const fn merge_ledger_entry_size_within_limit(encoded_len: usize) -> bool {
    encoded_len <= MAX_MERGE_LEDGER_ENTRY_BYTES
}

/// Return whether an exact canonical execution-batch length fits its reserved envelope.
#[must_use]
pub const fn merge_execution_batch_size_within_limit(encoded_len: usize) -> bool {
    encoded_len <= MAX_MERGE_EXECUTION_BATCH_BYTES
}

/// Proof of possession for one signer selected by a merge QC bitmap.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeSignerProof {
    /// Signer index in [`MergeQuorumCertificate::validator_set`].
    pub signer: ValidatorIndex,
    /// BLS proof of possession for the indexed validator key.
    pub proof_of_possession: Vec<u8>,
}

/// Canonical active lane incarnation and first eligible proposal height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneBinding {
    /// Active lane identifier.
    pub lane_id: LaneId,
    /// Dataspace bound to the active lane.
    pub dataspace_id: DataSpaceId,
    /// Canonical hash of the active lane configuration.
    pub lane_config_hash: Hash,
    /// Active incarnation commitment.
    pub incarnation: Hash,
    /// First global proposal height eligible to use this incarnation.
    pub activation_height: u64,
}

/// BFT quorum certificate produced by the merge committee for a merge-ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeQuorumCertificate {
    /// View number in which the merge committee formed the certificate.
    pub view: u64,
    /// Epoch identifier active when the merge entry was finalized.
    pub epoch_id: u64,
    /// Exact global block height authorized to carry this merge entry.
    pub carrier_height: u64,
    /// Exact canonical parent authorized for the global carrier.
    pub carrier_parent_hash: HashOf<BlockHeader>,
    /// Domain-separated digest of the chain identifier sealed by this QC.
    pub chain_id_digest: Hash,
    /// Version of the canonical validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Hash of the exact ordered validator set used by the signer bitmap.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Exact historical validator set used to form this QC.
    pub validator_set: Vec<PeerId>,
    /// Bitmap encoding of participating validators (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// Exact, canonical signer PoPs required to verify the aggregate signature after restart.
    pub signer_proofs: Vec<MergeSignerProof>,
    /// Aggregate signature bytes covering the serialized merge entry payload.
    pub aggregate_signature: Vec<u8>,
    /// Deterministic transcript hash used when verifying the certificate.
    pub message_digest: Hash,
}

impl MergeQuorumCertificate {
    /// Construct a new quorum certificate using explicit fields.
    pub fn new(
        view: u64,
        epoch_id: u64,
        carrier_height: u64,
        carrier_parent_hash: HashOf<BlockHeader>,
        chain_id_digest: Hash,
        validator_set_hash_version: u16,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Vec<PeerId>,
        signers_bitmap: Vec<u8>,
        signer_proofs: Vec<MergeSignerProof>,
        aggregate_signature: Vec<u8>,
        message_digest: Hash,
    ) -> Self {
        Self {
            view,
            epoch_id,
            carrier_height,
            carrier_parent_hash,
            chain_id_digest,
            validator_set_hash_version,
            validator_set_hash,
            validator_set,
            signers_bitmap,
            signer_proofs,
            aggregate_signature,
            message_digest,
        }
    }
}

/// Signature share emitted by a merge-committee member.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeCommitteeSignature {
    /// Merge-ledger entry epoch/height being signed.
    pub epoch_id: u64,
    /// Merge-committee view index aligned with lane tips for this entry.
    pub view: u64,
    /// Signer index in the merge-committee roster.
    pub signer: ValidatorIndex,
    /// Deterministic transcript hash used when verifying the signature.
    pub message_digest: Hash,
    /// BLS signature payload for the merge entry digest.
    pub bls_sig: Vec<u8>,
}

/// Canonical per-lane snapshot recorded inside a merge-ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneSnapshot {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active incarnation commitment for the lane-local height namespace.
    pub lane_incarnation: Hash,
    /// First global proposal height eligible to use this incarnation.
    pub incarnation_activation_height: u64,
    /// Global proposal height that selected this lane-local block.
    pub proposal_height: u64,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Lane-local block height represented by this snapshot.
    pub lane_block_height: u64,
    /// Canonical tip hash for the lane.
    pub tip_hash: HashOf<BlockHeader>,
    /// Merge-hint root associated with this lane snapshot.
    pub merge_hint_root: Hash,
    /// Exact settlement payload durably replayed after merge-log recovery.
    pub settlement_commitment: LaneBlockCommitment,
    /// Hash binding [`Self::settlement_commitment`] to the certified relay.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
    /// Exact authenticated relay envelope from which every snapshot field was derived.
    ///
    /// This is optional at the codec layer so malformed/missing-proof candidates can be decoded
    /// and rejected deterministically. Production admission requires `Some` and never consults a
    /// follower's opportunistic relay cache.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub relay_envelope: Option<LaneRelayEnvelope>,
}

/// Proof of possession retained for a signer of an embedded lane-local QC.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneSignerProof {
    /// BLS public key whose ownership is proven.
    pub public_key: PublicKey,
    /// BLS proof of possession for `public_key`.
    pub proof_of_possession: Vec<u8>,
}

/// One commit-certified lane block and its deterministic execution transcript.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneExecution {
    /// Canonical framed Norito bytes of the complete producer-authenticated
    /// payload, availability certificate, NewView chain, and lane QCs.
    pub source_bundle: Vec<u8>,
    /// Hash of `source_bundle`, used for holder fetch and corruption checks.
    pub source_bundle_hash: Hash,
    /// Commit-certified lane-local proposal.
    pub proposal: LaneBlockProposalV1,
    /// Producer-authenticated origin proposal whose view owns the durable queue
    /// reservations; `proposal` may be a later quorum-authorized view.
    pub origin_proposal: LaneBlockProposalV1,
    /// Prepare QC that also proves quorum payload availability.
    pub prepare_qc: LaneBlockQcV1,
    /// Commit QC authorizing execution of the proposal.
    pub commit_qc: LaneBlockQcV1,
    /// Canonically ordered PoPs needed to verify both embedded QCs after restart.
    pub signer_proofs: Vec<MergeLaneSignerProof>,
    /// Chain binding of the producer-authenticated autonomous payload.
    pub autonomous_chain_id_hash: Hash,
    /// Consensus epoch bound into the autonomous payload.
    pub autonomous_epoch: u64,
    /// Canonical digest of the exact autonomous executable payload.
    pub autonomous_payload_hash: Hash,
    /// Entrypoint hashes in descriptor order.
    pub entrypoint_hashes: Vec<Hash>,
    /// Exact entrypoints executed in descriptor order.
    pub entrypoints: Vec<TransactionEntrypoint>,
    /// Canonical Norito encodings of the exact durable queue reservation keys,
    /// aligned one-for-one with `entrypoints`.
    ///
    /// The concrete reservation type belongs to `iroha_core`, so the data model
    /// retains its exact canonical bytes without introducing a dependency cycle.
    pub reservation_keys: Vec<Vec<u8>>,
    /// Canonical Norito encodings of the complete routing plans, aligned
    /// one-for-one with `entrypoints` and `reservation_keys`.
    pub routing_plans: Vec<Vec<u8>>,
    /// Producer-authenticated native-AMX receipts aligned one-for-one with
    /// `entrypoints` and routing plans (`Some` only for native-AMX plans).
    pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
    /// Deterministic result hashes in descriptor order.
    pub result_hashes: Vec<Hash>,
    /// Exact deterministic results in descriptor order.
    pub results: Vec<TransactionResult>,
    /// Settlement, Nexus-fee, and native-AMX evidence derived by the same execution.
    pub settlement_commitment: LaneBlockCommitment,
    /// Canonical hash of `settlement_commitment`.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
}

/// Merge-committee-certified total-order execution batch.
///
/// The batch is self-contained so a node that crashes after the merge-log append
/// but before WSV publication can replay the exact transition without trusting
/// lane sidecars or local QC arrival order.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeExecutionBatch {
    /// Schema version. Version one is the only currently valid value.
    pub version: u8,
    /// Canonical committed WSV height from which execution starts.
    pub base_state_height: u64,
    /// Canonical committed WSV identity from which execution starts.
    pub base_state_hash: HashOf<BlockHeader>,
    /// Deterministic block context used for stateful execution.
    pub application_block_header: BlockHeader,
    /// Lane executions in the canonical merge total order.
    pub lanes: Vec<MergeLaneExecution>,
    /// Number of ordered entrypoint/result leaves committed by this batch.
    pub entrypoint_count: u64,
    /// Canonical Merkle root of entrypoint hashes in lane/batch execution order.
    pub entrypoint_merkle_root: HashOf<MerkleTree<TransactionEntrypoint>>,
    /// Canonical Merkle root of result hashes in the same execution order.
    pub result_merkle_root: HashOf<MerkleTree<TransactionResult>>,
    /// Merkle-style domain hash of the ordered lane execution transcripts.
    pub execution_root: Hash,
    /// Canonical root of execution, settlement, event, and transaction-membership
    /// effects before deterministic replay markers are added.
    pub application_write_set_root: Hash,
    /// Canonical root of the complete ordered WSV write set, including the
    /// deterministic replay markers derived from the stable batch identity.
    pub write_set_root: Hash,
    /// Expected post-state identity derived from the canonical base WSV and write-set root.
    pub expected_post_state_hash: HashOf<BlockHeader>,
    /// Canonical digest of every preceding field in this batch.
    pub batch_hash: Hash,
}

/// Ordered log entry produced by the merge ledger.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLedgerEntry {
    /// Epoch in which the entry was committed.
    pub epoch_id: u64,
    /// Canonical hash of the active lane catalog used for admission.
    pub lane_catalog_hash: Hash,
    /// Canonical exact active lane bindings used for historical verification.
    pub active_lanes: Vec<MergeLaneBinding>,
    /// Root of the canonical `(lane_id, incarnation)` set.
    pub incarnation_root: Hash,
    /// Root of the canonical `(lane_id, incarnation, activation_height)` set.
    pub activation_root: Hash,
    /// Canonical per-lane snapshots included in this merge entry.
    pub lane_snapshots: Vec<MergeLaneSnapshot>,
    /// Deterministic reduction of `merge_hint_roots` across all lanes.
    pub global_state_root: Hash,
    /// Merge committee quorum certificate sealing the entry.
    pub merge_qc: MergeQuorumCertificate,
    /// Optional commit-certified autonomous lane execution batch.
    ///
    /// This field is trailing so pre-feature persisted entries decode through
    /// the Norito default without shifting legacy positional fields.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub execution_batch: Option<MergeExecutionBatch>,
}

impl MergeLedgerEntry {
    /// Return the canonical framed Norito bytes used by hash-addressed entry sidecars.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        norito::to_bytes(self).expect("merge-ledger entry must have a canonical Norito encoding")
    }

    /// Return the single canonical, domain-separated hash used by compact block
    /// references, pending sidecars, committed carrier indexes, and receipts.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        let bytes = self.canonical_bytes();
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            MERGE_LEDGER_ENTRY_HASH_DOMAIN,
            bytes.as_slice(),
        ]))
    }

    /// Return the exact canonical framed length committed by compact references.
    #[must_use]
    pub fn canonical_encoded_len(&self) -> u64 {
        u64::try_from(self.canonical_bytes().len()).unwrap_or(u64::MAX)
    }

    /// Return whether this entry fits the protocol-wide full-entry envelope.
    #[must_use]
    pub fn canonical_size_within_limit(&self) -> bool {
        merge_ledger_entry_size_within_limit(self.canonical_bytes().len())
    }

    /// Number of lanes represented by this entry.
    #[must_use]
    pub fn lane_count(&self) -> usize {
        self.lane_snapshots.len()
    }

    /// Canonical lane tips derived from [`Self::lane_snapshots`].
    #[must_use]
    pub fn lane_tips(&self) -> Vec<HashOf<BlockHeader>> {
        self.lane_snapshots
            .iter()
            .map(|snapshot| snapshot.tip_hash)
            .collect()
    }

    /// Canonical merge-hint roots derived from [`Self::lane_snapshots`].
    #[must_use]
    pub fn merge_hint_roots(&self) -> Vec<Hash> {
        self.lane_snapshots
            .iter()
            .map(|snapshot| snapshot.merge_hint_root)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[derive(Encode)]
    struct LegacyMergeLedgerEntry {
        epoch_id: u64,
        lane_catalog_hash: Hash,
        active_lanes: Vec<MergeLaneBinding>,
        incarnation_root: Hash,
        activation_root: Hash,
        lane_snapshots: Vec<MergeLaneSnapshot>,
        global_state_root: Hash,
        merge_qc: MergeQuorumCertificate,
    }

    fn sample_tip(label: &[u8]) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn sample_hash(label: &[u8]) -> Hash {
        Hash::new(label)
    }

    fn sample_settlement(
        lane_id: LaneId,
        lane_incarnation: Hash,
        dataspace_id: DataSpaceId,
        block_height: u64,
    ) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height,
            lane_id,
            lane_incarnation,
            dataspace_id,
            tx_count: 0,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        }
    }

    #[test]
    fn merge_protocol_size_limits_are_inclusive_and_reserve_entry_headroom() {
        assert!(merge_execution_batch_size_within_limit(
            MAX_MERGE_EXECUTION_BATCH_BYTES
        ));
        assert!(!merge_execution_batch_size_within_limit(
            MAX_MERGE_EXECUTION_BATCH_BYTES + 1
        ));
        assert!(merge_ledger_entry_size_within_limit(
            MAX_MERGE_LEDGER_ENTRY_BYTES
        ));
        assert!(!merge_ledger_entry_size_within_limit(
            MAX_MERGE_LEDGER_ENTRY_BYTES + 1
        ));
        assert_eq!(
            MAX_MERGE_LEDGER_ENTRY_BYTES - MAX_MERGE_EXECUTION_BATCH_BYTES,
            4 * 1024 * 1024
        );
    }

    #[test]
    fn reserved_entry_headroom_fits_maximum_execution_committee_and_lane_bindings() {
        const MAX_ACTIVE_LANES: usize = 1_024;
        const MAX_MERGE_VALIDATORS: usize = 4_096;
        const BLS_PROOF_BYTES: usize = 96;

        let keypair =
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal).expect("BLS fixture keypair");
        let peer = PeerId::new(keypair.public_key().clone());
        let validator_set = vec![peer; MAX_MERGE_VALIDATORS];
        let validator_set_hash = HashOf::new(&validator_set);
        let signer_proofs = (0..MAX_MERGE_VALIDATORS)
            .map(|index| MergeSignerProof {
                signer: u32::try_from(index).expect("validator index fits"),
                proof_of_possession: vec![0; BLS_PROOF_BYTES],
            })
            .collect();
        let active_lanes = (0..MAX_ACTIVE_LANES)
            .map(|index| {
                let index_u64 = u64::try_from(index).expect("lane index fits u64");
                MergeLaneBinding {
                    lane_id: LaneId::new(u32::try_from(index).expect("lane index fits")),
                    dataspace_id: DataSpaceId::new(index_u64),
                    lane_config_hash: sample_hash(&index_u64.to_le_bytes()),
                    incarnation: sample_hash(&index_u64.saturating_add(1).to_le_bytes()),
                    activation_height: 1,
                }
            })
            .collect();
        let entry = MergeLedgerEntry {
            epoch_id: 1,
            lane_catalog_hash: sample_hash(b"max-overhead-catalog"),
            active_lanes,
            incarnation_root: sample_hash(b"max-overhead-incarnations"),
            activation_root: sample_hash(b"max-overhead-activations"),
            lane_snapshots: Vec::new(),
            global_state_root: sample_hash(b"max-overhead-global-root"),
            merge_qc: MergeQuorumCertificate::new(
                0,
                1,
                1,
                HashOf::from_untyped_unchecked(sample_hash(b"max-overhead-parent")),
                sample_hash(b"max-overhead-chain"),
                1,
                validator_set_hash,
                validator_set,
                vec![0xff; MAX_MERGE_VALIDATORS.div_ceil(8)],
                signer_proofs,
                vec![0; BLS_PROOF_BYTES],
                sample_hash(b"max-overhead-message"),
            ),
            execution_batch: None,
        };
        let envelope_overhead = entry.canonical_bytes().len();
        assert!(
            envelope_overhead <= MAX_MERGE_LEDGER_ENTRY_BYTES - MAX_MERGE_EXECUTION_BATCH_BYTES,
            "maximum committee/lane envelope {envelope_overhead} exceeds reserved headroom"
        );
    }

    #[test]
    fn merge_entry_roundtrip() {
        let qc = MergeQuorumCertificate::new(
            7,
            3,
            4,
            HashOf::from_untyped_unchecked(sample_hash(b"carrier-parent")),
            sample_hash(b"chain"),
            1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            vec![0b1010_1010],
            Vec::new(),
            vec![0xAA, 0xBB, 0xCC],
            sample_hash(b"qc-digest"),
        );
        let entry = MergeLedgerEntry {
            epoch_id: 3,
            lane_catalog_hash: sample_hash(b"catalog"),
            active_lanes: vec![
                MergeLaneBinding {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(7),
                    lane_config_hash: sample_hash(b"config-1"),
                    incarnation: sample_hash(b"incarnation-1"),
                    activation_height: 1,
                },
                MergeLaneBinding {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(9),
                    lane_config_hash: sample_hash(b"config-2"),
                    incarnation: sample_hash(b"incarnation-2"),
                    activation_height: 1,
                },
            ],
            incarnation_root: sample_hash(b"incarnation-root"),
            activation_root: sample_hash(b"activation-root"),
            lane_snapshots: vec![
                MergeLaneSnapshot {
                    lane_id: LaneId::new(1),
                    lane_incarnation: sample_hash(b"incarnation-1"),
                    incarnation_activation_height: 1,
                    proposal_height: 2,
                    dataspace_id: DataSpaceId::new(7),
                    lane_block_height: 11,
                    tip_hash: sample_tip(b"lane-0"),
                    merge_hint_root: sample_hash(b"root-0"),
                    settlement_commitment: sample_settlement(
                        LaneId::new(1),
                        sample_hash(b"incarnation-1"),
                        DataSpaceId::new(7),
                        11,
                    ),
                    settlement_hash: HashOf::new(&sample_settlement(
                        LaneId::new(1),
                        sample_hash(b"incarnation-1"),
                        DataSpaceId::new(7),
                        11,
                    )),
                    relay_envelope: None,
                },
                MergeLaneSnapshot {
                    lane_id: LaneId::new(2),
                    lane_incarnation: sample_hash(b"incarnation-2"),
                    incarnation_activation_height: 1,
                    proposal_height: 2,
                    dataspace_id: DataSpaceId::new(9),
                    lane_block_height: 14,
                    tip_hash: sample_tip(b"lane-1"),
                    merge_hint_root: sample_hash(b"root-1"),
                    settlement_commitment: sample_settlement(
                        LaneId::new(2),
                        sample_hash(b"incarnation-2"),
                        DataSpaceId::new(9),
                        14,
                    ),
                    settlement_hash: HashOf::new(&sample_settlement(
                        LaneId::new(2),
                        sample_hash(b"incarnation-2"),
                        DataSpaceId::new(9),
                        14,
                    )),
                    relay_envelope: None,
                },
            ],
            execution_batch: None,
            global_state_root: sample_hash(b"global"),
            merge_qc: qc.clone(),
        };

        assert_eq!(entry.lane_count(), 2);
        assert_eq!(entry.lane_tips().len(), 2);
        assert_eq!(entry.merge_hint_roots().len(), 2);

        let encoded = Encode::encode(&entry);
        let decoded = MergeLedgerEntry::decode(&mut &encoded[..])
            .expect("merge entry rounds trips through Norito");
        assert_eq!(decoded, entry);

        let legacy = LegacyMergeLedgerEntry {
            epoch_id: entry.epoch_id,
            lane_catalog_hash: entry.lane_catalog_hash,
            active_lanes: entry.active_lanes.clone(),
            incarnation_root: entry.incarnation_root,
            activation_root: entry.activation_root,
            lane_snapshots: entry.lane_snapshots.clone(),
            global_state_root: entry.global_state_root,
            merge_qc: entry.merge_qc.clone(),
        };
        let legacy_encoded = legacy.encode();
        let mut legacy_slice = legacy_encoded.as_slice();
        let decoded_legacy = MergeLedgerEntry::decode(&mut legacy_slice)
            .expect("legacy merge entry must decode with a missing trailing batch");
        assert_eq!(decoded_legacy.execution_batch, None);
        assert_eq!(decoded_legacy.epoch_id, entry.epoch_id);
        assert_eq!(decoded_legacy.merge_qc, entry.merge_qc);
    }

    #[test]
    fn quorum_certificate_roundtrip() {
        let qc = MergeQuorumCertificate::new(
            11,
            5,
            6,
            HashOf::from_untyped_unchecked(sample_hash(b"carrier-parent")),
            sample_hash(b"chain"),
            1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            vec![0xFF, 0x00],
            Vec::new(),
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            sample_hash(b"digest"),
        );
        let encoded = Encode::encode(&qc);
        let decoded = MergeQuorumCertificate::decode(&mut &encoded[..])
            .expect("quorum certificate round-trips");
        assert_eq!(decoded, qc);
    }

    #[test]
    fn merge_committee_signature_roundtrip() {
        let signature = MergeCommitteeSignature {
            epoch_id: 9,
            view: 1,
            signer: 2,
            message_digest: sample_hash(b"merge-digest"),
            bls_sig: vec![0x10, 0x20, 0x30],
        };
        let encoded = Encode::encode(&signature);
        let decoded = MergeCommitteeSignature::decode(&mut &encoded[..])
            .expect("merge signature round-trips");
        assert_eq!(decoded, signature);
    }
}
