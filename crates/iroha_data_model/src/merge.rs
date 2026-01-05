//! Merge-ledger data structures.
//!
//! The merge ledger records a compact ordered log of lane tips along with the
//! deterministic global reduction state used to finalize world state updates.
//! These DTOs provide the on-wire and persistence representations of merge
//! entries. See `docs/source/merge_ledger.md` for the normative behaviour the
//! runtime must enforce when producing and validating these records.

use iroha_crypto::{Hash, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::block::{BlockHeader, consensus::ValidatorIndex};

/// BFT quorum certificate produced by the merge committee for a merge-ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeQuorumCertificate {
    /// View number in which the merge committee formed the certificate.
    pub view: u64,
    /// Epoch identifier active when the merge entry was finalized.
    pub epoch_id: u64,
    /// Bitmap encoding of participating validators (LSB-first).
    pub signers_bitmap: Vec<u8>,
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
        signers_bitmap: Vec<u8>,
        aggregate_signature: Vec<u8>,
        message_digest: Hash,
    ) -> Self {
        Self {
            view,
            epoch_id,
            signers_bitmap,
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

/// Ordered log entry produced by the merge ledger.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLedgerEntry {
    /// Epoch in which the entry was committed.
    pub epoch_id: u64,
    /// Canonical tips for each execution lane.
    pub lane_tips: Vec<HashOf<BlockHeader>>,
    /// Merge-hint Poseidon roots aligned with the lane tips.
    pub merge_hint_roots: Vec<Hash>,
    /// Deterministic reduction of `merge_hint_roots` across all lanes.
    pub global_state_root: Hash,
    /// Merge committee quorum certificate sealing the entry.
    pub merge_qc: MergeQuorumCertificate,
}

impl MergeLedgerEntry {
    /// Number of lanes represented by this entry.
    #[must_use]
    pub fn lane_count(&self) -> usize {
        self.lane_tips.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tip(label: &[u8]) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn sample_hash(label: &[u8]) -> Hash {
        Hash::new(label)
    }

    #[test]
    fn merge_entry_roundtrip() {
        let qc = MergeQuorumCertificate::new(
            7,
            3,
            vec![0b1010_1010],
            vec![0xAA, 0xBB, 0xCC],
            sample_hash(b"qc-digest"),
        );
        let entry = MergeLedgerEntry {
            epoch_id: 3,
            lane_tips: vec![sample_tip(b"lane-0"), sample_tip(b"lane-1")],
            merge_hint_roots: vec![sample_hash(b"root-0"), sample_hash(b"root-1")],
            global_state_root: sample_hash(b"global"),
            merge_qc: qc.clone(),
        };

        assert_eq!(entry.lane_count(), 2);

        let encoded = Encode::encode(&entry);
        let decoded = MergeLedgerEntry::decode(&mut &encoded[..])
            .expect("merge entry rounds trips through Norito");
        assert_eq!(decoded, entry);
    }

    #[test]
    fn quorum_certificate_roundtrip() {
        let qc = MergeQuorumCertificate::new(
            11,
            5,
            vec![0xFF, 0x00],
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
