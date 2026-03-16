//! Merge-ledger helpers (reduction, validation, and related utilities).

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    merge::{MergeLaneSnapshot, MergeLedgerEntry, MergeQuorumCertificate},
};
use iroha_zkp_halo2::poseidon;
use norito::codec::Encode;

/// Domain separator applied to the merge-hint reduction payloads.
const MERGE_REDUCE_DOMAIN_TAG: &[u8] = b"iroha:merge:reduce:v1\0";
/// Domain separator applied to merge-committee signature payloads.
const MERGE_QC_DOMAIN_TAG: &[u8] = b"iroha:merge:qc:v1\0";

/// Merge-ledger entry data required for signature payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeLedgerCandidate {
    /// Epoch/height for the merge entry.
    pub epoch_id: u64,
    /// Merge committee view derived from lane tips.
    pub view: u64,
    /// Canonical per-lane snapshots for this merge entry.
    pub lane_snapshots: Vec<MergeLaneSnapshot>,
    /// Deterministic reduction of `merge_hint_roots` across all lanes.
    pub global_state_root: Hash,
}

impl MergeLedgerCandidate {
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
            lane_snapshots: self.lane_snapshots,
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
            lane_snapshots: entry.lane_snapshots.clone(),
            global_state_root: entry.global_state_root,
        }
    }
}

#[derive(Encode)]
struct MergeLedgerSignPayload {
    view: u64,
    epoch_id: u64,
    lane_snapshots: Vec<MergeLaneSnapshot>,
    global_state_root: Hash,
}

/// Compute the deterministic message digest for merge-committee signatures.
#[must_use]
pub fn merge_qc_message_digest(chain_id: &ChainId, candidate: &MergeLedgerCandidate) -> Hash {
    let payload = MergeLedgerSignPayload {
        view: candidate.view,
        epoch_id: candidate.epoch_id,
        lane_snapshots: candidate.lane_snapshots.clone(),
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
        let candidate = MergeLedgerCandidate {
            epoch_id: 7,
            view: 3,
            lane_snapshots: vec![MergeLaneSnapshot {
                lane_id: iroha_data_model::nexus::LaneId::new(1),
                dataspace_id: iroha_data_model::nexus::DataSpaceId::new(7),
                lane_block_height: 9,
                tip_hash: HashOf::from_untyped_unchecked(Hash::new(b"lane-0")),
                merge_hint_root: Hash::new(b"hint-0"),
            }],
            global_state_root: Hash::new(b"global"),
        };
        let chain_id: ChainId = "nexus-merge".parse().expect("chain id parses");
        let digest_a = merge_qc_message_digest(&chain_id, &candidate);
        let digest_b = merge_qc_message_digest(&chain_id, &candidate);
        assert_eq!(digest_a, digest_b);
    }
}
