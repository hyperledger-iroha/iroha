//! Canonical balanced-Merkle primitives for finalized Kagemusha top-up anchors.

use iroha_crypto::Hash;

use super::{KAGEMUSHA_TOPUP_FINALITY_MAX_ANCHORS_PER_BLOCK_V2, KagemushaTopUpAnchorMerkleProofV2};

/// Execution-witness key tag for a finalized Kagemusha top-up anchor.
pub const KAGEMUSHA_TOPUP_ANCHOR_WITNESS_KEY_TAG_V2: u8 = 0xD2;

const TOPUP_NODE_DOMAIN_V2: &[u8] = b"iroha:kagemusha:v2:topup-node";
const TOPUP_EMPTY_DOMAIN_V2: &[u8] = b"iroha:kagemusha:v2:topup-empty";

/// Hash one canonical top-up operation/digest pair as a balanced-tree leaf.
///
/// Returns `None` when either identity is zero.
#[must_use]
pub fn kagemusha_topup_anchor_leaf_hash_v2(
    operation_id: [u8; 32],
    anchor_digest: [u8; 32],
) -> Option<Hash> {
    if operation_id == [0; 32] || anchor_digest == [0; 32] {
        return None;
    }
    let mut key = Vec::with_capacity(1 + operation_id.len());
    key.push(KAGEMUSHA_TOPUP_ANCHOR_WITNESS_KEY_TAG_V2);
    key.extend_from_slice(&operation_id);
    let key_hash = Hash::new(key);
    let value_hash = Hash::new(anchor_digest);
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(0x00);
    preimage.extend_from_slice(key_hash.as_ref());
    preimage.extend_from_slice(value_hash.as_ref());
    Some(Hash::new(preimage))
}

/// Return the canonical padding hash for the balanced top-up tree.
#[must_use]
pub fn kagemusha_topup_anchor_empty_hash_v2() -> Hash {
    Hash::new(TOPUP_EMPTY_DOMAIN_V2)
}

/// Hash one ordered pair of balanced top-up tree nodes at `level`.
#[must_use]
pub fn kagemusha_topup_anchor_node_hash_v2(level: u16, left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(
        TOPUP_NODE_DOMAIN_V2.len() + 1 + core::mem::size_of::<u16>() + 2 * Hash::LENGTH,
    );
    preimage.extend_from_slice(TOPUP_NODE_DOMAIN_V2);
    preimage.push(0);
    preimage.extend_from_slice(&level.to_le_bytes());
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}

/// Reconstruct the committed root selected by one top-up anchor path.
#[must_use]
pub fn kagemusha_topup_anchor_root_from_merkle_proof_v2(
    operation_id: [u8; 32],
    anchor_digest: [u8; 32],
    path: &KagemushaTopUpAnchorMerkleProofV2,
) -> Option<Hash> {
    if path.leaf_count == 0
        || path.leaf_count > KAGEMUSHA_TOPUP_FINALITY_MAX_ANCHORS_PER_BLOCK_V2
        || path.leaf_index >= path.leaf_count
    {
        return None;
    }
    let expected_depth = path.leaf_count.next_power_of_two().trailing_zeros() as usize;
    if path.siblings.len() != expected_depth {
        return None;
    }
    let Some(mut current) = kagemusha_topup_anchor_leaf_hash_v2(operation_id, anchor_digest) else {
        return None;
    };
    let mut index = path.leaf_index;
    for (level, sibling) in path.siblings.iter().copied().enumerate() {
        if sibling[Hash::LENGTH - 1] & 1 == 0 {
            return None;
        }
        let Ok(level) = u16::try_from(level) else {
            return None;
        };
        let sibling = Hash::prehashed(sibling);
        current = if index & 1 == 0 {
            kagemusha_topup_anchor_node_hash_v2(level, current, sibling)
        } else {
            kagemusha_topup_anchor_node_hash_v2(level, sibling, current)
        };
        index /= 2;
    }
    Some(current)
}

/// Verify one top-up anchor path against the Commit-QC-authenticated tree root.
#[must_use]
pub fn verify_kagemusha_topup_anchor_merkle_proof_v2(
    operation_id: [u8; 32],
    anchor_digest: [u8; 32],
    path: &KagemushaTopUpAnchorMerkleProofV2,
    expected_root: Hash,
) -> bool {
    kagemusha_topup_anchor_root_from_merkle_proof_v2(operation_id, anchor_digest, path)
        == Some(expected_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::consensus_v2::ExecutionCommitment;

    fn assert_hash_hex(value: Hash, expected: &str) {
        assert_eq!(hex::encode(value.as_ref()), expected);
    }

    #[test]
    fn topup_anchor_merkle_protocol_vectors_are_stable() {
        let first =
            kagemusha_topup_anchor_leaf_hash_v2([1; 32], [2; 32]).expect("first top-up leaf");
        assert_hash_hex(
            first,
            "e493225a37007b02facd936d65c57af9194d65587166a5911c1428d390ef4af1",
        );
        let second =
            kagemusha_topup_anchor_leaf_hash_v2([3; 32], [4; 32]).expect("second top-up leaf");
        let first_pair = kagemusha_topup_anchor_node_hash_v2(0, first, second);
        assert_hash_hex(
            first_pair,
            "09ac4bc6859dc58f10e184621f084b09d5f8cecdf1b4cc4bf9c751187b327aa7",
        );
        let empty = kagemusha_topup_anchor_empty_hash_v2();
        assert_hash_hex(
            empty,
            "57462dc0f74cde13c8be60a15ec2d90818c59115d7cb129ec4504e7d8ae38d0b",
        );
        let third =
            kagemusha_topup_anchor_leaf_hash_v2([5; 32], [6; 32]).expect("third top-up leaf");
        assert_hash_hex(
            third,
            "ae16847856596f22112623a3924cb836201613376590f0664e0034f4a630b8e9",
        );
        let three_leaf_root = kagemusha_topup_anchor_node_hash_v2(
            1,
            first_pair,
            kagemusha_topup_anchor_node_hash_v2(0, third, empty),
        );
        assert_hash_hex(
            three_leaf_root,
            "263fdd9b1ee01f8805f15d98ea06232e07975e47cef364fd56fbfb7bb6483961",
        );
        let ordinary_writes_root = Hash::new(b"kagemusha-vector-ordinary-writes");
        assert_hash_hex(
            ordinary_writes_root,
            "bdb8028472626e75e23cd5454ea24a236b8a4ff7832b06c3e461506e1c789cf1",
        );
        assert_hash_hex(
            ExecutionCommitment::topup_post_state_root(3, ordinary_writes_root, three_leaf_root),
            "62923dba058b441ca6d87ea67f5d096d916cdf5af1d5105e18b0d32c38d721f1",
        );
    }

    #[test]
    fn topup_anchor_merkle_proof_authenticates_every_sibling() {
        let left = kagemusha_topup_anchor_leaf_hash_v2([1; 32], [2; 32]).expect("left top-up leaf");
        let right =
            kagemusha_topup_anchor_leaf_hash_v2([3; 32], [4; 32]).expect("right top-up leaf");
        let root = kagemusha_topup_anchor_node_hash_v2(0, left, right);
        let path = KagemushaTopUpAnchorMerkleProofV2 {
            leaf_index: 0,
            leaf_count: 2,
            siblings: vec![right.into()],
        };
        assert!(verify_kagemusha_topup_anchor_merkle_proof_v2(
            [1; 32], [2; 32], &path, root
        ));

        let mut forged = path;
        forged.siblings[0][0] ^= 1;
        assert!(!verify_kagemusha_topup_anchor_merkle_proof_v2(
            [1; 32], [2; 32], &forged, root
        ));
    }

    #[test]
    fn topup_anchor_merkle_proof_rejects_noncanonical_sibling_hashes() {
        let leaf = kagemusha_topup_anchor_leaf_hash_v2([1; 32], [2; 32]).expect("top-up leaf");
        let mut sibling: [u8; 32] = kagemusha_topup_anchor_empty_hash_v2().into();
        sibling[31] &= !1;
        let path = KagemushaTopUpAnchorMerkleProofV2 {
            leaf_index: 0,
            leaf_count: 2,
            siblings: vec![sibling],
        };
        let root =
            kagemusha_topup_anchor_node_hash_v2(0, leaf, kagemusha_topup_anchor_empty_hash_v2());
        assert!(path.validate().is_err());
        assert!(!verify_kagemusha_topup_anchor_merkle_proof_v2(
            [1; 32], [2; 32], &path, root
        ));
    }
}
