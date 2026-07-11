//! Merge-ledger helpers (reduction, validation, and related utilities).

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    merge::{
        LaneDrainCertificateV1, MergeExecutionBatch, MergeLaneBinding, MergeLaneExecution,
        MergeLaneSnapshot, MergeLedgerEntry, MergeQuorumCertificate,
    },
    nexus::LaneConfig,
    peer::PeerId,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};
use iroha_zkp_halo2::poseidon;
use norito::codec::{Decode, Encode};

/// Domain separator applied to the merge-hint reduction payloads.
const MERGE_REDUCE_DOMAIN_TAG: &[u8] = b"iroha:merge:reduce:v1\0";
/// Domain separator applied to merge-committee signature payloads.
const MERGE_QC_DOMAIN_TAG: &[u8] = b"iroha:merge:qc:v1\0";
/// Domain separator for the chain identity embedded into durable merge QCs.
const MERGE_CHAIN_ID_DOMAIN_TAG: &[u8] = b"iroha:merge:chain-id:v1\0";
/// Domain separator for exact lane-incarnation activation commitments.
const MERGE_ACTIVATION_ROOT_DOMAIN_TAG: &[u8] = b"iroha:merge:lane-activations:v1\0";
/// Domain separator for individual lane configuration commitments.
const MERGE_LANE_CONFIG_DOMAIN_TAG: &[u8] = b"iroha:merge:lane-config:v1\0";
/// Domain separator for one lane execution transcript.
const MERGE_LANE_EXECUTION_DOMAIN_TAG: &[u8] = b"iroha:merge:lane-execution:v1\0";
/// Domain separator for the ordered execution root.
const MERGE_EXECUTION_ROOT_DOMAIN_TAG: &[u8] = b"iroha:merge:execution-root:v1\0";
/// Domain separator for the stable, pre-write-set batch identity used by replay markers.
const MERGE_EXECUTION_IDENTITY_DOMAIN_TAG: &[u8] = b"iroha:merge:execution-identity:v1\0";
/// Domain separator for the deterministic post-state transition commitment.
const MERGE_POST_STATE_DOMAIN_TAG: &[u8] = b"iroha:merge:post-state:v1\0";
/// Domain separator for a complete merge execution batch.
const MERGE_EXECUTION_BATCH_DOMAIN_TAG: &[u8] = b"iroha:merge:execution-batch:v1\0";
const MERGE_CANDIDATE_BODY_DOMAIN_TAG: &[u8] = b"iroha:merge:candidate-body:v1\0";

/// Merge-ledger entry data required for signature payloads.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct MergeLedgerCandidate {
    /// Epoch/height for the merge entry.
    pub epoch_id: u64,
    /// Merge committee view derived from lane tips.
    pub view: u64,
    /// Exact global block height authorized to carry the candidate.
    pub carrier_height: u64,
    /// Exact canonical parent authorized for the global carrier.
    pub carrier_parent_hash: HashOf<BlockHeader>,
    /// Canonical hash of the active catalog used to assemble the candidate.
    pub lane_catalog_hash: Hash,
    /// Exact active lane binding set bound to the catalog hash.
    pub active_lanes: Vec<MergeLaneBinding>,
    /// Canonical active-incarnation root.
    pub incarnation_root: Hash,
    /// Canonical incarnation-activation root.
    pub activation_root: Hash,
    /// Canonical per-lane snapshots for this merge entry.
    pub lane_snapshots: Vec<MergeLaneSnapshot>,
    /// Optional commit-certified autonomous lane execution batch.
    pub execution_batch: Option<MergeExecutionBatch>,
    /// Lane-committee drain certificates globally ordered by this candidate.
    pub lane_drain_certificates: Vec<LaneDrainCertificateV1>,
    /// Deterministic reduction of `merge_hint_roots` across all lanes.
    pub global_state_root: Hash,
}

impl MergeLedgerCandidate {
    /// Return the canonical framed Norito body transferred before QC signing.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        norito::to_bytes(self).expect("merge candidate must have a canonical Norito encoding")
    }

    /// Return the domain-separated hash of [`Self::canonical_bytes`].
    #[must_use]
    pub fn canonical_hash(&self) -> Hash {
        let bytes = self.canonical_bytes();
        Hash::new_from_chunks(&[MERGE_CANDIDATE_BODY_DOMAIN_TAG, bytes.as_slice()])
    }

    /// Return whether the candidate fits the shared full-entry transfer cap.
    #[must_use]
    pub fn canonical_size_within_limit(&self) -> bool {
        self.canonical_bytes().len() <= iroha_data_model::merge::MAX_MERGE_LEDGER_ENTRY_BYTES
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

    /// Convert this candidate into a full merge-ledger entry with the supplied QC.
    #[must_use]
    pub fn into_entry(self, merge_qc: MergeQuorumCertificate) -> MergeLedgerEntry {
        MergeLedgerEntry {
            epoch_id: self.epoch_id,
            lane_catalog_hash: self.lane_catalog_hash,
            active_lanes: self.active_lanes,
            incarnation_root: self.incarnation_root,
            activation_root: self.activation_root,
            lane_snapshots: self.lane_snapshots,
            execution_batch: self.execution_batch,
            lane_drain_certificates: self.lane_drain_certificates,
            global_state_root: self.global_state_root,
            merge_qc,
        }
    }
}

impl From<&MergeLedgerEntry> for MergeLedgerCandidate {
    fn from(entry: &MergeLedgerEntry) -> Self {
        Self {
            epoch_id: entry.epoch_id,
            view: entry.merge_qc.view,
            carrier_height: entry.merge_qc.carrier_height,
            carrier_parent_hash: entry.merge_qc.carrier_parent_hash,
            lane_catalog_hash: entry.lane_catalog_hash,
            active_lanes: entry.active_lanes.clone(),
            incarnation_root: entry.incarnation_root,
            activation_root: entry.activation_root,
            lane_snapshots: entry.lane_snapshots.clone(),
            execution_batch: entry.execution_batch.clone(),
            lane_drain_certificates: entry.lane_drain_certificates.clone(),
            global_state_root: entry.global_state_root,
        }
    }
}

/// Return the canonical framed Norito bytes used by hash-addressed merge-entry
/// sidecars and compact globally ordered references.
#[must_use]
pub fn canonical_merge_ledger_entry_bytes(entry: &MergeLedgerEntry) -> Vec<u8> {
    entry.canonical_bytes()
}

/// Compute the domain-separated digest of a complete merge-ledger entry,
/// including its merge QC and execution batch.
#[must_use]
pub fn merge_ledger_entry_hash(entry: &MergeLedgerEntry) -> HashOf<MergeLedgerEntry> {
    entry.canonical_hash()
}

/// Return the exact canonical framed byte length committed by a compact
/// merge-entry reference.
#[must_use]
pub fn merge_ledger_entry_encoded_len(entry: &MergeLedgerEntry) -> u64 {
    entry.canonical_encoded_len()
}

/// Verify the hash and exact byte length of a caller-resolved merge sidecar.
#[must_use]
pub fn merge_ledger_entry_reference_matches(
    entry: &MergeLedgerEntry,
    expected_hash: HashOf<MergeLedgerEntry>,
    expected_encoded_len: u64,
) -> bool {
    entry.canonical_encoded_len() == expected_encoded_len && entry.canonical_hash() == expected_hash
}

#[derive(Encode)]
struct MergeLedgerSignPayload {
    chain_id_digest: Hash,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    view: u64,
    epoch_id: u64,
    carrier_height: u64,
    carrier_parent_hash: HashOf<BlockHeader>,
    lane_catalog_hash: Hash,
    active_lanes: Vec<MergeLaneBinding>,
    incarnation_root: Hash,
    activation_root: Hash,
    lane_snapshots: Vec<MergeLaneSnapshot>,
    execution_batch: Option<MergeExecutionBatch>,
    lane_drain_certificates: Vec<LaneDrainCertificateV1>,
    global_state_root: Hash,
}

/// Compute the deterministic message digest for merge-committee signatures.
#[must_use]
pub fn merge_qc_message_digest(
    chain_id: &ChainId,
    candidate: &MergeLedgerCandidate,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
) -> Hash {
    let payload = MergeLedgerSignPayload {
        chain_id_digest: merge_chain_id_digest(chain_id),
        validator_set_hash_version,
        validator_set_hash,
        view: candidate.view,
        epoch_id: candidate.epoch_id,
        carrier_height: candidate.carrier_height,
        carrier_parent_hash: candidate.carrier_parent_hash,
        lane_catalog_hash: candidate.lane_catalog_hash,
        active_lanes: candidate.active_lanes.clone(),
        incarnation_root: candidate.incarnation_root,
        activation_root: candidate.activation_root,
        lane_snapshots: candidate.lane_snapshots.clone(),
        execution_batch: candidate.execution_batch.clone(),
        lane_drain_certificates: candidate.lane_drain_certificates.clone(),
        global_state_root: candidate.global_state_root,
    };
    let payload_bytes = payload.encode();
    let mut preimage = Vec::with_capacity(
        MERGE_QC_DOMAIN_TAG.len() + chain_id.as_str().len() + payload_bytes.len(),
    );
    preimage.extend_from_slice(MERGE_QC_DOMAIN_TAG);
    preimage.extend_from_slice(chain_id.as_str().as_bytes());
    preimage.extend_from_slice(&payload_bytes);
    Hash::new(preimage)
}

/// Compute the stable chain identity embedded in every durable merge QC.
#[must_use]
pub fn merge_chain_id_digest(chain_id: &ChainId) -> Hash {
    Hash::new_from_chunks(&[MERGE_CHAIN_ID_DOMAIN_TAG, chain_id.as_str().as_bytes()])
}

/// Compute the canonical activation root for an already canonical incarnation list.
#[must_use]
pub fn merge_activation_root(active_lanes: &[MergeLaneBinding]) -> Hash {
    let encoded = active_lanes.to_vec().encode();
    Hash::new_from_chunks(&[MERGE_ACTIVATION_ROOT_DOMAIN_TAG, encoded.as_slice()])
}

/// Compute the canonical configuration hash embedded in an active merge binding.
#[must_use]
pub fn merge_lane_config_hash(lane: &LaneConfig) -> Hash {
    let encoded = lane.encode();
    Hash::new_from_chunks(&[MERGE_LANE_CONFIG_DOMAIN_TAG, encoded.as_slice()])
}

/// Compute the canonical digest of one lane execution transcript.
#[must_use]
pub fn merge_lane_execution_hash(execution: &MergeLaneExecution) -> Hash {
    let encoded = execution.encode();
    Hash::new_from_chunks(&[MERGE_LANE_EXECUTION_DOMAIN_TAG, encoded.as_slice()])
}

/// Compute the ordered root of all lane execution transcripts in a batch.
#[must_use]
pub fn merge_execution_root(executions: &[MergeLaneExecution]) -> Hash {
    let transcript_hashes = executions
        .iter()
        .map(merge_lane_execution_hash)
        .collect::<Vec<_>>();
    let encoded = transcript_hashes.encode();
    Hash::new_from_chunks(&[MERGE_EXECUTION_ROOT_DOMAIN_TAG, encoded.as_slice()])
}

/// Strip carrier payload roots while retaining every block-context field that
/// can affect deterministic transaction admission/execution (height, parent,
/// ledger time, and view). This avoids a hash cycle with the compact merge
/// reference while preventing replay under a synthetic zero timestamp/view.
#[must_use]
pub fn merge_application_header_from_carrier(carrier: &BlockHeader) -> BlockHeader {
    BlockHeader::new(
        carrier.height(),
        carrier.prev_block_hash(),
        None,
        None,
        u64::try_from(carrier.creation_time().as_millis()).unwrap_or(u64::MAX),
        carrier.view_change_index(),
    )
}

/// Return entrypoint hashes in the exact lane/batch execution order used by
/// merge-sidecar inclusion proofs.
#[must_use]
pub fn merge_execution_entrypoint_hashes(
    executions: &[MergeLaneExecution],
) -> Vec<HashOf<TransactionEntrypoint>> {
    executions
        .iter()
        .flat_map(|execution| {
            execution
                .entrypoints
                .iter()
                .map(|entrypoint| entrypoint.hash())
        })
        .collect()
}

/// Return result hashes in the exact lane/batch execution order used by
/// merge-sidecar inclusion proofs.
#[must_use]
pub fn merge_execution_result_hashes(
    executions: &[MergeLaneExecution],
) -> Vec<HashOf<TransactionResult>> {
    executions
        .iter()
        .flat_map(|execution| execution.results.iter().map(|result| result.hash()))
        .collect()
}

/// Compute the canonical ordered entrypoint Merkle root for a non-empty batch.
#[must_use]
pub fn merge_execution_entrypoint_merkle_root(
    executions: &[MergeLaneExecution],
) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
    merge_execution_entrypoint_hashes(executions)
        .into_iter()
        .collect::<MerkleTree<TransactionEntrypoint>>()
        .root()
}

/// Compute the canonical ordered result Merkle root for a non-empty batch.
#[must_use]
pub fn merge_execution_result_merkle_root(
    executions: &[MergeLaneExecution],
) -> Option<HashOf<MerkleTree<TransactionResult>>> {
    merge_execution_result_hashes(executions)
        .into_iter()
        .collect::<MerkleTree<TransactionResult>>()
        .root()
}

#[derive(Encode)]
struct MergePostStatePreimage {
    base_state_height: u64,
    base_state_hash: HashOf<BlockHeader>,
    write_set_root: Hash,
}

/// Compute the deterministic post-state identity represented by a batch.
///
/// `base_state_hash` is the canonical committed WSV snapshot hash, not the latest
/// global block hash. `write_set_root` commits every changed key/value in the
/// ordered execution overlay (plus direct transaction membership). Together they
/// form a canonical post-state identity that detects divergent writes even when
/// two executions return equal success/failure result vectors.
#[must_use]
pub fn merge_expected_post_state_hash(
    base_state_height: u64,
    base_state_hash: HashOf<BlockHeader>,
    write_set_root: Hash,
) -> HashOf<BlockHeader> {
    let encoded = MergePostStatePreimage {
        base_state_height,
        base_state_hash,
        write_set_root,
    }
    .encode();
    HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
        MERGE_POST_STATE_DOMAIN_TAG,
        encoded.as_slice(),
    ]))
}

#[derive(Encode)]
struct MergeExecutionBatchPreimage {
    version: u8,
    base_state_height: u64,
    base_state_hash: HashOf<BlockHeader>,
    application_block_header: BlockHeader,
    lanes: Vec<MergeLaneExecution>,
    entrypoint_count: u64,
    entrypoint_merkle_root: HashOf<MerkleTree<TransactionEntrypoint>>,
    result_merkle_root: HashOf<MerkleTree<TransactionResult>>,
    execution_root: Hash,
    application_write_set_root: Hash,
    write_set_root: Hash,
    expected_post_state_hash: HashOf<BlockHeader>,
}

/// Compute the canonical digest of a complete execution batch.
#[must_use]
pub fn merge_execution_batch_hash(batch: &MergeExecutionBatch) -> Hash {
    let encoded = MergeExecutionBatchPreimage {
        version: batch.version,
        base_state_height: batch.base_state_height,
        base_state_hash: batch.base_state_hash,
        application_block_header: batch.application_block_header.clone(),
        lanes: batch.lanes.clone(),
        entrypoint_count: batch.entrypoint_count,
        entrypoint_merkle_root: batch.entrypoint_merkle_root,
        result_merkle_root: batch.result_merkle_root,
        execution_root: batch.execution_root,
        application_write_set_root: batch.application_write_set_root,
        write_set_root: batch.write_set_root,
        expected_post_state_hash: batch.expected_post_state_hash,
    }
    .encode();
    Hash::new_from_chunks(&[MERGE_EXECUTION_BATCH_DOMAIN_TAG, encoded.as_slice()])
}

#[derive(Encode)]
struct MergeExecutionIdentityPreimage {
    version: u8,
    base_state_height: u64,
    base_state_hash: HashOf<BlockHeader>,
    application_block_header: BlockHeader,
    entrypoint_count: u64,
    entrypoint_merkle_root: HashOf<MerkleTree<TransactionEntrypoint>>,
    result_merkle_root: HashOf<MerkleTree<TransactionResult>>,
    execution_root: Hash,
    application_write_set_root: Hash,
}

/// Compute the stable pre-marker identity of a merge execution batch.
///
/// Replay markers use this identity instead of the final batch hash because the
/// markers themselves are part of the final write set. Excluding only the
/// marker-dependent fields avoids a commitment cycle while the final batch hash
/// and merge QC still cover the complete marker-inclusive write-set root.
#[must_use]
pub fn merge_execution_batch_identity_hash(batch: &MergeExecutionBatch) -> Hash {
    let encoded = MergeExecutionIdentityPreimage {
        version: batch.version,
        base_state_height: batch.base_state_height,
        base_state_hash: batch.base_state_hash,
        application_block_header: batch.application_block_header.clone(),
        entrypoint_count: batch.entrypoint_count,
        entrypoint_merkle_root: batch.entrypoint_merkle_root,
        result_merkle_root: batch.result_merkle_root,
        execution_root: batch.execution_root,
        application_write_set_root: batch.application_write_set_root,
    }
    .encode();
    Hash::new_from_chunks(&[MERGE_EXECUTION_IDENTITY_DOMAIN_TAG, encoded.as_slice()])
}

/// Return whether all redundant execution-batch commitments are canonical.
#[must_use]
pub fn merge_execution_batch_commitments_match(batch: &MergeExecutionBatch) -> bool {
    let execution_root = merge_execution_root(&batch.lanes);
    let entrypoint_hashes = merge_execution_entrypoint_hashes(&batch.lanes);
    let result_hashes = merge_execution_result_hashes(&batch.lanes);
    execution_root == batch.execution_root
        && u64::try_from(entrypoint_hashes.len()).ok() == Some(batch.entrypoint_count)
        && result_hashes.len() == entrypoint_hashes.len()
        && entrypoint_hashes
            .into_iter()
            .collect::<MerkleTree<TransactionEntrypoint>>()
            .root()
            == Some(batch.entrypoint_merkle_root)
        && result_hashes
            .into_iter()
            .collect::<MerkleTree<TransactionResult>>()
            .root()
            == Some(batch.result_merkle_root)
        && merge_expected_post_state_hash(
            batch.base_state_height,
            batch.base_state_hash,
            batch.write_set_root,
        ) == batch.expected_post_state_hash
        && merge_execution_batch_hash(batch) == batch.batch_hash
}

/// Deterministically fold lane merge-hint roots into a single global root.
///
/// The reduction uses the Poseidon2 permutation (rate 2, capacity 1) with the
/// domain separator `iroha:merge:reduce:v1\0`. For a single-lane deployment the
/// reduction degenerates to identity so existing pipelines remain unchanged.
#[must_use]
pub fn reduce_merge_hint_roots(roots: &[Hash]) -> Hash {
    match roots.len() {
        0 => Hash::prehashed(poseidon::hash_bytes(MERGE_REDUCE_DOMAIN_TAG)),
        1 => roots[0],
        _ => {
            let mut acc = poseidon::hash_bytes(MERGE_REDUCE_DOMAIN_TAG);
            let mut payload = Vec::with_capacity(Hash::LENGTH * 2);
            for root in roots {
                payload.clear();
                payload.extend_from_slice(&acc);
                payload.extend_from_slice(root.as_ref());
                acc = poseidon::hash_bytes(&payload);
            }
            Hash::prehashed(acc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_merge_proof_roots_bind_order_and_duplicate_leaves() {
        let first = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(b"first"));
        let second = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(b"second"));
        let ordered = vec![first, second]
            .into_iter()
            .collect::<MerkleTree<TransactionEntrypoint>>()
            .root()
            .expect("non-empty root");
        let reordered = vec![second, first]
            .into_iter()
            .collect::<MerkleTree<TransactionEntrypoint>>()
            .root()
            .expect("non-empty root");
        let duplicated = vec![first, first]
            .into_iter()
            .collect::<MerkleTree<TransactionEntrypoint>>()
            .root()
            .expect("non-empty root");

        assert_ne!(ordered, reordered);
        assert_ne!(ordered, duplicated);
    }

    #[test]
    fn reduces_empty_sequence_to_domain_digest() {
        let reduced = reduce_merge_hint_roots(&[]);
        let expected = Hash::prehashed(poseidon::hash_bytes(MERGE_REDUCE_DOMAIN_TAG));
        assert_eq!(reduced, expected);
    }

    #[test]
    fn single_lane_is_identity() {
        let lane_root = Hash::prehashed([0xAA; Hash::LENGTH]);
        let reduced = reduce_merge_hint_roots(&[lane_root]);
        assert_eq!(reduced, lane_root);
    }

    #[test]
    fn multi_lane_matches_golden() {
        let lanes = [
            Hash::prehashed([0x01; Hash::LENGTH]),
            Hash::prehashed([0x02; Hash::LENGTH]),
            Hash::prehashed([0x03; Hash::LENGTH]),
        ];
        let reduced = reduce_merge_hint_roots(&lanes);
        let mut acc = poseidon::hash_bytes(MERGE_REDUCE_DOMAIN_TAG);
        let mut payload = Vec::with_capacity(Hash::LENGTH * 2);
        for root in &lanes {
            payload.clear();
            payload.extend_from_slice(&acc);
            payload.extend_from_slice(root.as_ref());
            acc = poseidon::hash_bytes(&payload);
        }
        let expected = Hash::prehashed(acc);
        assert_eq!(reduced, expected);
    }

    #[test]
    fn merge_qc_message_digest_is_deterministic() {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            consensus::VALIDATOR_SET_HASH_VERSION_V1,
            merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        };

        let lane_id = iroha_data_model::nexus::LaneId::new(1);
        let lane_incarnation = Hash::new(b"merge-test-lane-incarnation");
        let dataspace_id = iroha_data_model::nexus::DataSpaceId::new(7);
        let settlement_commitment = iroha_data_model::block::consensus::LaneBlockCommitment {
            block_height: 9,
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
        };
        let active_lanes = vec![MergeLaneBinding {
            lane_id,
            dataspace_id,
            lane_config_hash: Hash::new(b"config"),
            incarnation: lane_incarnation,
            activation_height: 1,
        }];
        let candidate = MergeLedgerCandidate {
            epoch_id: 7,
            view: 3,
            carrier_height: 10,
            carrier_parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"carrier-parent")),
            lane_catalog_hash: Hash::new(b"catalog"),
            incarnation_root: Hash::new(b"incarnation-root"),
            activation_root: merge_activation_root(&active_lanes),
            active_lanes,
            lane_snapshots: vec![MergeLaneSnapshot {
                lane_id,
                lane_incarnation,
                incarnation_activation_height: 1,
                proposal_height: 9,
                dataspace_id,
                lane_block_height: 9,
                tip_hash: HashOf::from_untyped_unchecked(Hash::new(b"lane-0")),
                merge_hint_root: Hash::new(b"hint-0"),
                settlement_hash: HashOf::new(&settlement_commitment),
                settlement_commitment,
                relay_envelope: None,
            }],
            execution_batch: None,
            lane_drain_certificates: Vec::new(),
            global_state_root: Hash::new(b"global"),
        };
        let chain_id: ChainId = "nexus-merge".parse().expect("chain id parses");
        let validator_set = Vec::<PeerId>::new();
        let validator_set_hash = HashOf::new(&validator_set);
        let digest_a = merge_qc_message_digest(&chain_id, &candidate, 1, validator_set_hash);
        let digest_b = merge_qc_message_digest(&chain_id, &candidate, 1, validator_set_hash);
        assert_eq!(digest_a, digest_b);

        let drain_keypair = KeyPair::try_from_seed(
            b"merge-digest-drain-validator".to_vec(),
            Algorithm::BlsNormal,
        )
        .expect("derive drain validator fixture");
        let drain_validator = PeerId::new(drain_keypair.public_key().clone());
        let drain_validators = vec![drain_validator];
        let certificate = LaneDrainCertificateV1 {
            body: LaneDrainCertificateBodyV1 {
                version: 1,
                intent: LaneDrainIntentV1 {
                    version: 1,
                    chain_id_digest: merge_chain_id_digest(&chain_id),
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    close_global_height: 8,
                    initial_merged_lane_height: 9,
                    initial_merged_descriptor_hash: Some(Hash::new(b"drain-initial")),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&drain_validators),
                    validator_set: drain_validators.clone(),
                    validator_count: 1,
                    min_quorum: 1,
                },
                final_lane_block_height: 9,
                final_lane_block_descriptor_hash: Some(Hash::new(b"drain-final")),
            },
            validator_set: drain_validators,
            signers_bitmap: vec![1],
            signer_proofs: Vec::new(),
            aggregate_signature: vec![0x55; 96],
        };
        let mut drain_candidate = candidate;
        drain_candidate.lane_snapshots.clear();
        drain_candidate.global_state_root = reduce_merge_hint_roots(&[]);
        drain_candidate.lane_drain_certificates = vec![certificate];
        let drain_digest =
            merge_qc_message_digest(&chain_id, &drain_candidate, 1, validator_set_hash);
        drain_candidate.lane_drain_certificates[0].aggregate_signature[0] ^= 0xFF;
        assert_ne!(
            merge_qc_message_digest(&chain_id, &drain_candidate, 1, validator_set_hash,),
            drain_digest,
            "the merge QC digest must bind every carried drain-certificate byte"
        );
    }
}
