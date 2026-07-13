//! Merge-ledger helpers (reduction, validation, and related utilities).

use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    ChainId,
    block::{BlockHeader, CertifiedMergeLedgerReference},
    da::commitment::DaProofScheme,
    merge::{
        MergeExecutionBatch, MergeLaneBinding, MergeLaneExecution, MergeLaneSnapshot,
        MergeLedgerEntry, MergeQuorumCertificate,
    },
    nexus::{DataSpaceId, LaneConfig, LaneId, LaneStorageProfile, LaneVisibility},
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
const MERGE_LANE_CONFIG_DOMAIN_TAG: &[u8] = b"iroha:merge:lane-config:v2\0";
/// Layout version for the consensus-relevant lane configuration projection.
const MERGE_LANE_CONFIG_PROJECTION_VERSION: u16 = 1;
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

/// Stable admission error used when a compact merge reference projects retired
/// autonomous execution data.
pub(crate) const RETIRED_MERGE_EXECUTION_PROJECTION: &str =
    "autonomous merge execution is retired; only settlement merge entries are accepted";

/// Return whether a compact merge reference advertises any retired execution
/// projection, including a deliberately partial projection.
#[must_use]
pub(crate) fn merge_reference_has_execution_projection(
    reference: &CertifiedMergeLedgerReference,
) -> bool {
    reference.execution_batch_hash.is_some()
        || reference.entrypoint_count.is_some()
        || reference.entrypoint_merkle_root.is_some()
        || reference.result_merkle_root.is_some()
        || reference.base_state_height.is_some()
        || reference.base_state_hash.is_some()
}

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
    /// Retired autonomous execution projection retained for canonical decoding.
    /// Production admission requires this field to be `None`.
    pub execution_batch: Option<MergeExecutionBatch>,
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

#[derive(Encode)]
struct MergeLaneConfigConsensusProjection {
    version: u16,
    id: LaneId,
    dataspace_id: DataSpaceId,
    visibility: LaneVisibility,
    lane_type: Option<String>,
    governance: Option<String>,
    settlement: Option<String>,
    storage: LaneStorageProfile,
    proof_scheme: DaProofScheme,
    metadata: BTreeMap<String, String>,
}

impl MergeLaneConfigConsensusProjection {
    fn from_lane(lane: &LaneConfig) -> Self {
        // Keep this destructuring exhaustive so adding a field to `LaneConfig`
        // requires an explicit decision about whether consensus must bind it.
        let LaneConfig {
            id,
            dataspace_id,
            alias: _,
            description: _,
            visibility,
            lane_type,
            governance,
            settlement,
            storage,
            proof_scheme,
            metadata,
        } = lane;

        Self {
            version: MERGE_LANE_CONFIG_PROJECTION_VERSION,
            id: *id,
            dataspace_id: *dataspace_id,
            visibility: *visibility,
            lane_type: lane_type.clone(),
            governance: governance.clone(),
            settlement: settlement.clone(),
            storage: *storage,
            proof_scheme: *proof_scheme,
            metadata: metadata.clone(),
        }
    }
}

/// Compute the canonical consensus configuration hash embedded in an active merge binding.
///
/// Human-facing aliases and descriptions remain committed by the exact catalog
/// hash, but do not alter the lane consensus projection.
#[must_use]
pub fn merge_lane_config_hash(lane: &LaneConfig) -> Hash {
    let encoded = MergeLaneConfigConsensusProjection::from_lane(lane).encode();
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
    use std::num::NonZeroU32;

    use iroha_data_model::nexus::{LaneCatalog, LaneLifecycleParameterV1};

    use super::*;

    fn exact_catalog_hash(lane: LaneConfig) -> Hash {
        let catalog = LaneCatalog::new(NonZeroU32::new(1).expect("one is non-zero"), vec![lane])
            .expect("single default-id lane must form a valid catalog");
        LaneLifecycleParameterV1::catalog_hash(&catalog)
    }

    fn settlement_reference_fixture() -> CertifiedMergeLedgerReference {
        let validator_set = Vec::<PeerId>::new();
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"settlement-sidecar")),
            encoded_len: 1,
            epoch_id: 1,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate {
                view: 0,
                epoch_id: 1,
                carrier_height: 1,
                carrier_parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent")),
                chain_id_digest: Hash::new(b"chain"),
                validator_set_hash_version: 1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                signers_bitmap: Vec::new(),
                signer_proofs: Vec::new(),
                aggregate_signature: Vec::new(),
                message_digest: Hash::new(b"merge-qc"),
            },
        }
    }

    #[test]
    fn execution_projection_detector_rejects_partial_and_full_shapes() {
        let settlement = settlement_reference_fixture();
        assert!(!merge_reference_has_execution_projection(&settlement));

        let mut partial = settlement.clone();
        partial.base_state_height = Some(0);
        assert!(merge_reference_has_execution_projection(&partial));

        let mut full = settlement;
        full.execution_batch_hash = Some(Hash::new(b"batch"));
        full.entrypoint_count = Some(1);
        full.entrypoint_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(b"entries")));
        full.result_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(b"results")));
        full.base_state_height = Some(0);
        full.base_state_hash = Some(HashOf::from_untyped_unchecked(Hash::new(b"base")));
        assert!(merge_reference_has_execution_projection(&full));
    }

    #[test]
    fn merge_lane_config_hash_excludes_display_fields_but_exact_catalog_hash_keeps_them() {
        let base = LaneConfig {
            description: Some("Primary settlement lane".to_owned()),
            ..LaneConfig::default()
        };
        let mut renamed = base.clone();
        renamed.alias = "renamed-primary".to_owned();
        let mut redescribed = base.clone();
        redescribed.description = Some("Updated operator-facing description".to_owned());

        let consensus_hash = merge_lane_config_hash(&base);
        assert_eq!(merge_lane_config_hash(&renamed), consensus_hash);
        assert_eq!(merge_lane_config_hash(&redescribed), consensus_hash);

        let exact_hash = exact_catalog_hash(base);
        assert_ne!(exact_catalog_hash(renamed), exact_hash);
        assert_ne!(exact_catalog_hash(redescribed), exact_hash);
    }

    #[test]
    fn merge_lane_config_hash_commits_every_functional_field_and_all_metadata() {
        let mut base = LaneConfig {
            lane_type: Some("retail".to_owned()),
            governance: Some("boi".to_owned()),
            settlement: Some("gross".to_owned()),
            ..LaneConfig::default()
        };
        base.metadata
            .insert("consensus.max_txs".to_owned(), "100".to_owned());
        base.metadata
            .insert("operator.policy".to_owned(), "strict".to_owned());

        let mut changed_id = base.clone();
        changed_id.id = LaneId::new(1);
        let mut changed_dataspace = base.clone();
        changed_dataspace.dataspace_id = DataSpaceId::new(7);
        let mut changed_visibility = base.clone();
        changed_visibility.visibility = LaneVisibility::Restricted;
        let mut changed_lane_type = base.clone();
        changed_lane_type.lane_type = Some("wholesale".to_owned());
        let mut changed_governance = base.clone();
        changed_governance.governance = Some("committee".to_owned());
        let mut changed_settlement = base.clone();
        changed_settlement.settlement = Some("net".to_owned());
        let mut changed_storage = base.clone();
        changed_storage.storage = LaneStorageProfile::SplitReplica;
        let mut changed_proof_scheme = base.clone();
        changed_proof_scheme.proof_scheme = DaProofScheme::KzgBls12_381;
        let mut changed_metadata_value = base.clone();
        changed_metadata_value
            .metadata
            .insert("consensus.max_txs".to_owned(), "101".to_owned());
        let mut changed_metadata_entries = base.clone();
        changed_metadata_entries
            .metadata
            .insert("consensus.timeout_ms".to_owned(), "500".to_owned());

        let baseline_hash = merge_lane_config_hash(&base);
        for (field, changed) in [
            ("id", changed_id),
            ("dataspace_id", changed_dataspace),
            ("visibility", changed_visibility),
            ("lane_type", changed_lane_type),
            ("governance", changed_governance),
            ("settlement", changed_settlement),
            ("storage", changed_storage),
            ("proof_scheme", changed_proof_scheme),
            ("metadata value", changed_metadata_value),
            ("metadata entries", changed_metadata_entries),
        ] {
            assert_ne!(
                merge_lane_config_hash(&changed),
                baseline_hash,
                "changing {field} must change the merge lane consensus hash"
            );
        }
    }

    #[test]
    fn merge_lane_config_hash_matches_protocol_golden() {
        let mut lane = LaneConfig {
            id: LaneId::new(3),
            dataspace_id: DataSpaceId::new(9),
            alias: "operator-display-name".to_owned(),
            description: Some("operator display description".to_owned()),
            visibility: LaneVisibility::Restricted,
            lane_type: Some("retail".to_owned()),
            governance: Some("boi".to_owned()),
            settlement: Some("gross".to_owned()),
            storage: LaneStorageProfile::SplitReplica,
            proof_scheme: DaProofScheme::KzgBls12_381,
            metadata: BTreeMap::new(),
        };
        lane.metadata
            .insert("consensus.max_txs".to_owned(), "100".to_owned());
        lane.metadata
            .insert("operator.policy".to_owned(), "strict".to_owned());

        assert_eq!(
            merge_lane_config_hash(&lane).to_string(),
            "3cfede3ff6488a005392fe462b04975fdc81214ab52ae2bd90203c5271130027"
        );
    }

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
        let lane_id = iroha_data_model::nexus::LaneId::new(1);
        let lane_incarnation = Hash::new(b"merge-test-lane-incarnation");
        let dataspace_id = iroha_data_model::nexus::DataSpaceId::new(7);
        let settlement_commitment = iroha_data_model::block::consensus::LaneBlockCommitment {
            block_height: 9,
            lane_id,
            lane_incarnation,
            dataspace_id,
            tx_count: 0,
            total_local_amount: "0".parse().expect("valid settlement quantity"),
            total_xor_due: "0".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
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
                settlement_hash: iroha_data_model::nexus::compute_settlement_hash(
                    &settlement_commitment,
                )
                .expect("test settlement should hash canonically"),
                settlement_commitment,
                relay_envelope: None,
            }],
            execution_batch: None,
            global_state_root: Hash::new(b"global"),
        };
        let chain_id: ChainId = "nexus-merge".parse().expect("chain id parses");
        let validator_set = Vec::<PeerId>::new();
        let validator_set_hash = HashOf::new(&validator_set);
        let digest_a = merge_qc_message_digest(&chain_id, &candidate, 1, validator_set_hash);
        let digest_b = merge_qc_message_digest(&chain_id, &candidate, 1, validator_set_hash);
        assert_eq!(digest_a, digest_b);
    }
}
