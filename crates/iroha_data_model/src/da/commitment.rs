use std::{fmt, io::Cursor, str::FromStr};

use iroha_crypto::{Hash, HashOf, Signature};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    core::{self as ncore, DecodeFromSlice},
    to_bytes,
};
use thiserror::Error;

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    da::types::{BlobDigest, RetentionPolicy, StorageTicketId},
    nexus::{DataSpaceId, LaneId},
    sorafs::pin_registry::ManifestDigest,
};

/// Proof scheme used to authenticate DA commitments.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "type", content = "value"))]
pub enum DaProofScheme {
    /// Merkle proof over SHA-256 chunk digests.
    #[default]
    MerkleSha256,
    /// Polynomial commitment using BLS12-381 KZG.
    KzgBls12_381,
}

/// Policy snapshot describing the proof scheme expected for a lane.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaProofPolicy {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Dataspace identifier associated with the lane.
    pub dataspace_id: DataSpaceId,
    /// Human-readable lane alias.
    pub alias: String,
    /// Proof scheme enforced for DA commitments on this lane.
    pub proof_scheme: DaProofScheme,
}

/// Versioned bundle of proof policies for all configured lanes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaProofPolicyBundle {
    /// Bundle layout version.
    pub version: u16,
    /// Deterministic hash over the ordered policies in this bundle.
    pub policy_hash: Hash,
    /// Ordered proof policies referenced by lanes.
    pub policies: Vec<DaProofPolicy>,
}

impl DaProofPolicyBundle {
    /// Initial version identifier for proof policy bundles.
    pub const VERSION_V1: u16 = 1;

    /// Construct a bundle using the latest supported version.
    #[must_use]
    pub fn new(policies: Vec<DaProofPolicy>) -> Self {
        Self {
            version: Self::VERSION_V1,
            policy_hash: hash_policies(&policies),
            policies,
        }
    }
}

fn hash_policies(policies: &[DaProofPolicy]) -> Hash {
    let bytes = to_bytes(&policies.to_vec())
        .expect("serializing proof policies with Norito must not fail at runtime");
    Hash::new(bytes)
}

impl DaProofScheme {
    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MerkleSha256 => "merkle_sha256",
            Self::KzgBls12_381 => "kzg_bls12_381",
        }
    }
}

impl fmt::Display for DaProofScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error surfaced when parsing [`DaProofScheme`] from a string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid DA proof scheme `{0}`")]
pub struct DaProofSchemeParseError(pub String);

impl FromStr for DaProofScheme {
    type Err = DaProofSchemeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "merkle_sha256" | "merkle-sha256" | "merkle" => Ok(Self::MerkleSha256),
            "kzg_bls12_381" | "kzg-bls12-381" | "kzg" => Ok(Self::KzgBls12_381),
            other => Err(DaProofSchemeParseError(other.to_string())),
        }
    }
}

/// Canonical DA commitment persisted in Nexus blocks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaCommitmentRecord {
    /// Lane the blob belongs to.
    pub lane_id: LaneId,
    /// Epoch the blob was scheduled for.
    pub epoch: u64,
    /// Monotonic sequence inside `(lane_id, epoch)`.
    pub sequence: u64,
    /// Client-declared blob identifier.
    pub client_blob_id: BlobDigest,
    /// Canonical manifest digest (BLAKE3 over encoded `DaManifestV1`).
    pub manifest_hash: ManifestDigest,
    /// Proof scheme expected for the target lane.
    pub proof_scheme: DaProofScheme,
    /// Merkle root over chunk digests for the blob.
    pub chunk_root: Hash,
    /// Optional KZG commitment accompanying the chunk root.
    pub kzg_commitment: Option<KzgCommitment>,
    /// Optional digest covering PDP/PoTR scheduling metadata.
    pub proof_digest: Option<Hash>,
    /// Retention summary applied to this blob.
    pub retention_class: RetentionClass,
    /// Storage ticket tying the blob to `SoraFS` replication state.
    pub storage_ticket: StorageTicketId,
    /// Signature issued by the Torii DA service key.
    pub acknowledgement_sig: Signature,
}

impl DaCommitmentRecord {
    /// Convenience constructor assembling a record with the default bundle version.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        client_blob_id: BlobDigest,
        manifest_hash: ManifestDigest,
        proof_scheme: DaProofScheme,
        chunk_root: Hash,
        kzg_commitment: Option<KzgCommitment>,
        proof_digest: Option<Hash>,
        retention_class: RetentionClass,
        storage_ticket: StorageTicketId,
        acknowledgement_sig: Signature,
    ) -> Self {
        Self {
            lane_id,
            epoch,
            sequence,
            client_blob_id,
            manifest_hash,
            proof_scheme,
            chunk_root,
            kzg_commitment,
            proof_digest,
            retention_class,
            storage_ticket,
            acknowledgement_sig,
        }
    }
}

/// Bundle embedded into `SignedBlockWire` and hashed inside `BlockHeader`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaCommitmentBundle {
    /// Bundle layout version.
    pub version: u16,
    /// Ordered commitment records contained in the block.
    pub commitments: Vec<DaCommitmentRecord>,
}

impl DaCommitmentBundle {
    /// Initial version identifier for on-chain bundles.
    pub const VERSION_V1: u16 = 1;

    /// Construct a bundle using the latest supported version.
    #[must_use]
    pub fn new(commitments: Vec<DaCommitmentRecord>) -> Self {
        Self {
            version: Self::VERSION_V1,
            commitments,
        }
    }

    /// Returns `true` if there are no commitments in the bundle.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commitments.is_empty()
    }

    /// Canonical Merkle root over the commitment records in this bundle.
    ///
    /// Leaves are `hash(norito::to_bytes(DaCommitmentRecord))`. Odd leaves are
    /// promoted unchanged to the next layer instead of being duplicated.
    #[must_use]
    pub fn merkle_root(&self) -> Option<Hash> {
        if self.commitments.is_empty() {
            return None;
        }

        let mut layer: Vec<Hash> = self.commitments.iter().map(commitment_leaf_hash).collect();
        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut iter = layer.chunks(2);
            for pair in iter.by_ref() {
                let combined = if pair.len() == 1 {
                    pair[0]
                } else {
                    let mut buf = Vec::with_capacity(Hash::LENGTH * 2);
                    buf.extend_from_slice(pair[0].as_ref());
                    buf.extend_from_slice(pair[1].as_ref());
                    Hash::new(buf)
                };
                next.push(combined);
            }
            layer = next;
        }

        layer.pop()
    }

    /// Hash the bundle with the canonical `Hash` function.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        // Encoding must not fail for well-formed bundles.
        let encoded =
            to_bytes(self).expect("DA commitment bundle Norito encoding must succeed for hashing");
        HashOf::<Self>::from_untyped_unchecked(Hash::new(encoded))
    }
}

impl Default for DaCommitmentBundle {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for DaCommitmentBundle {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let _guard = ncore::PayloadCtxGuard::enter(bytes);
        let mut cursor = Cursor::new(bytes);
        let decoded: DaCommitmentBundle = Decode::decode(&mut cursor)?;
        let used = usize::try_from(cursor.position()).map_err(|_| ncore::Error::LengthMismatch)?;
        Ok((decoded, used))
    }
}

/// KZG commitment wrapper used when blocks embed polynomial proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct KzgCommitment(
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))] pub [u8; 48],
);

impl KzgCommitment {
    /// Canonical zero commitment (identity point in compressed form).
    pub const ZERO_BYTES: [u8; 48] = [0; 48];

    /// Construct a commitment from raw bytes (BLS12-381 G1 compressed form).
    #[must_use]
    pub const fn new(bytes: [u8; 48]) -> Self {
        Self(bytes)
    }

    /// Access the raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 48] {
        &self.0
    }

    /// Convenience constructor returning the zero commitment.
    #[must_use]
    pub const fn zero() -> Self {
        Self(Self::ZERO_BYTES)
    }
}

impl Default for KzgCommitment {
    fn default() -> Self {
        Self::zero()
    }
}

impl<'a> DecodeFromSlice<'a> for KzgCommitment {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

/// Alias representing the retained policy class recorded on-chain.
pub type RetentionClass = RetentionPolicy;

/// Canonical key identifying a DA commitment across lanes/epochs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaCommitmentKey {
    /// Lane the blob belongs to.
    pub lane_id: LaneId,
    /// Epoch the blob was scheduled for.
    pub epoch: u64,
    /// Monotonic sequence inside `(lane_id, epoch)`.
    pub sequence: u64,
}

impl DaCommitmentKey {
    /// Build a key from an existing commitment record.
    #[must_use]
    pub fn from_record(record: &DaCommitmentRecord) -> Self {
        Self {
            lane_id: record.lane_id,
            epoch: record.epoch,
            sequence: record.sequence,
        }
    }
}

/// Location of a sealed DA commitment inside the blockchain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaCommitmentLocation {
    /// Height of the block that sealed the commitment.
    pub block_height: u64,
    /// Index within the commitment bundle for that block (0-based).
    pub index_in_bundle: u32,
}

/// Commitment record paired with its on-chain location.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaCommitmentWithLocation {
    /// Raw commitment stored on chain.
    pub commitment: DaCommitmentRecord,
    /// Position of this commitment within the chain.
    pub location: DaCommitmentLocation,
}

/// Direction of a sibling inside a binary Merkle tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "direction", content = "value"))]
pub enum MerkleDirection {
    /// Sibling hash is on the left.
    Left,
    /// Sibling hash is on the right.
    Right,
}

/// A single hop inside a Merkle proof path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MerklePathItem {
    /// Hash of the sibling node at this tree level.
    pub sibling: Hash,
    /// Whether the sibling sits to the left or right of the target node.
    pub direction: MerkleDirection,
}

/// Membership proof for a DA commitment inside a block bundle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaCommitmentProof {
    /// Commitment covered by the proof.
    pub commitment: DaCommitmentRecord,
    /// Position of the commitment inside the block bundle.
    pub location: DaCommitmentLocation,
    /// Hash of the full commitment bundle as stored in the block header.
    pub bundle_hash: HashOf<DaCommitmentBundle>,
    /// Total number of commitments in the bundle.
    pub bundle_len: u32,
    /// Merkle root derived from the ordered commitment list.
    pub root: Hash,
    /// Merkle path connecting the commitment leaf to `root`.
    pub path: Vec<MerklePathItem>,
}

/// Hash a commitment into its Merkle leaf value.
#[must_use]
pub fn commitment_leaf_hash(record: &DaCommitmentRecord) -> Hash {
    // Encoding a commitment must be infallible; treat failures as unreachable.
    let bytes = to_bytes(record).expect("DA commitment must encode");
    Hash::new(bytes)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::Hash;
    use norito::codec::{DecodeAll, encode_adaptive};

    use super::*;

    fn sample_record() -> DaCommitmentRecord {
        DaCommitmentRecord {
            lane_id: LaneId::new(7),
            epoch: 42,
            sequence: 3,
            client_blob_id: BlobDigest::new([0x11; 32]),
            manifest_hash: ManifestDigest::new([0x22; 32]),
            proof_scheme: DaProofScheme::MerkleSha256,
            chunk_root: Hash::prehashed([0x33; 32]),
            kzg_commitment: Some(KzgCommitment::new([0x44; 48])),
            proof_digest: Some(Hash::prehashed([0x55; 32])),
            retention_class: RetentionClass::default(),
            storage_ticket: StorageTicketId::new([0x66; 32]),
            acknowledgement_sig: Signature::from_bytes(&[0x77; 64]),
        }
    }

    #[test]
    fn commitment_round_trip() {
        let record = sample_record();
        let bytes = encode_adaptive(&record);
        let decoded = DaCommitmentRecord::decode_all(&mut bytes.as_slice()).expect("decode");
        assert_eq!(record, decoded);
    }

    #[test]
    fn bundle_round_trip() {
        let bundle = DaCommitmentBundle::new(vec![sample_record()]);
        let bytes = encode_adaptive(&bundle);
        let decoded = DaCommitmentBundle::decode_all(&mut bytes.as_slice()).expect("decode");
        assert_eq!(bundle, decoded);
        assert!(!bundle.is_empty());
    }

    #[test]
    fn merkle_root_returns_none_for_empty_bundle() {
        assert!(DaCommitmentBundle::default().merkle_root().is_none());
    }

    #[test]
    fn merkle_root_uses_single_leaf_without_hashing_upwards() {
        let record = sample_record();
        let leaf = commitment_leaf_hash(&record);
        let bundle = DaCommitmentBundle::new(vec![record]);

        assert_eq!(bundle.merkle_root(), Some(leaf));
    }

    #[test]
    fn merkle_root_promotes_last_leaf_on_odd_layers() {
        let mut records = Vec::new();
        // Three leaves -> level 1 has two nodes: hash(0 || 1) and leaf(2).
        for idx in 0..3 {
            let mut record = sample_record();
            record.sequence = idx;
            records.push(record);
        }
        let bundle = DaCommitmentBundle::new(records.clone());

        let leaves: Vec<_> = records.iter().map(commitment_leaf_hash).collect();
        let level1_left = {
            let mut buf = Vec::with_capacity(Hash::LENGTH * 2);
            buf.extend_from_slice(leaves[0].as_ref());
            buf.extend_from_slice(leaves[1].as_ref());
            Hash::new(buf)
        };
        let expected_root = {
            let mut buf = Vec::with_capacity(Hash::LENGTH * 2);
            buf.extend_from_slice(level1_left.as_ref());
            buf.extend_from_slice(leaves[2].as_ref());
            Hash::new(buf)
        };

        assert_eq!(bundle.merkle_root(), Some(expected_root));
    }

    #[test]
    fn canonical_hash_matches_encoded_bundle() {
        let record = sample_record();
        let bundle = DaCommitmentBundle::new(vec![record]);
        let encoded = to_bytes(&bundle).expect("encode bundle");
        let expected = HashOf::<DaCommitmentBundle>::from_untyped_unchecked(Hash::new(encoded));

        assert_eq!(bundle.canonical_hash(), expected);
    }

    #[test]
    fn proof_scheme_from_str_handles_aliases() {
        assert_eq!(
            DaProofScheme::from_str("merkle_sha256").expect("parse"),
            DaProofScheme::MerkleSha256
        );
        assert_eq!(
            DaProofScheme::from_str("kzg-bls12-381").expect("parse"),
            DaProofScheme::KzgBls12_381
        );
    }

    #[test]
    fn proof_scheme_from_str_rejects_unknown() {
        assert!(DaProofScheme::from_str("unknown-scheme").is_err());
    }

    #[test]
    fn proof_policy_bundle_hash_changes_on_drift() {
        let base = DaProofPolicy {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::new(11),
            alias: "lane-a".to_string(),
            proof_scheme: DaProofScheme::MerkleSha256,
        };
        let mut switched = base.clone();
        switched.proof_scheme = DaProofScheme::KzgBls12_381;

        let bundle_a = DaProofPolicyBundle::new(vec![base]);
        let bundle_b = DaProofPolicyBundle::new(vec![switched]);

        assert_ne!(bundle_a.policy_hash, bundle_b.policy_hash);
    }

    #[test]
    fn proof_policy_bundle_hash_stable_for_same_ordering() {
        let policy_a = DaProofPolicy {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            alias: "one".to_string(),
            proof_scheme: DaProofScheme::MerkleSha256,
        };
        let policy_b = DaProofPolicy {
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(2),
            alias: "two".to_string(),
            proof_scheme: DaProofScheme::KzgBls12_381,
        };

        let first = DaProofPolicyBundle::new(vec![policy_a.clone(), policy_b.clone()]);
        let second = DaProofPolicyBundle::new(vec![policy_a, policy_b]);

        assert_eq!(first.policy_hash, second.policy_hash);
        assert_eq!(first.version, DaProofPolicyBundle::VERSION_V1);
    }
}
