//! Deterministic ordering helpers for FASTPQ batches.
//!
//! Ordering commitments follow the staged plan in `docs/source/fastpq_plan.md`
//! and now use the canonical Poseidon2 sponge over Goldilocks with the
//! `fastpq:v1:ordering` domain tag.

use ::core::convert::TryInto as _;
use iroha_crypto::Hash;
use norito::core;

use crate::{Result, TransitionBatch, pack_bytes, poseidon};

/// Domain separation tag for ordering commitments.
const ORDERING_DOMAIN: &[u8] = b"fastpq:v1:ordering";

/// Compute the canonical ordering commitment for a batch.
///
/// # Errors
///
/// Propagates Norito serialization failures or parameter mismatches while
/// cloning the batch for canonical ordering.
pub fn ordering_hash(batch: &TransitionBatch) -> Result<Hash> {
    let mut canonical = batch.clone();
    canonical.sort();
    let encoded = core::to_bytes(&canonical.transitions)?;

    let domain_packed = pack_bytes(ORDERING_DOMAIN);
    let encoded_packed = pack_bytes(&encoded);

    let mut limbs = Vec::with_capacity(domain_packed.limbs.len() + encoded_packed.limbs.len() + 2);
    limbs.push(
        domain_packed
            .length
            .try_into()
            .expect("ordering domain length fits into field limb"),
    );
    limbs.extend(domain_packed.limbs);
    limbs.push(
        encoded_packed
            .length
            .try_into()
            .expect("encoded transition bytes length fits into field limb"),
    );
    limbs.extend(encoded_packed.limbs);

    let digest = poseidon::hash_field_elements(&limbs);
    let mut bytes = [0u8; Hash::LENGTH];
    bytes[..8].copy_from_slice(&digest.to_le_bytes());
    Ok(Hash::prehashed(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OperationKind, StateTransition};

    #[test]
    fn ordering_hash_stable_under_permutations() {
        let mut original =
            TransitionBatch::new("fastpq-lane-balanced", crate::PublicInputs::default());
        original.push(StateTransition::new(
            b"asset/a".to_vec(),
            vec![1],
            vec![2],
            OperationKind::Transfer,
        ));
        original.push(StateTransition::new(
            b"asset/a".to_vec(),
            vec![2],
            vec![3],
            OperationKind::Burn,
        ));
        original.push(StateTransition::new(
            b"asset/b".to_vec(),
            vec![5],
            vec![6],
            OperationKind::Mint,
        ));

        let mut permuted = original.clone();
        permuted.transitions.swap(0, 2);
        permuted.transitions.swap(0, 1);

        let h1 = ordering_hash(&original).expect("ordering hash");
        let h2 = ordering_hash(&permuted).expect("ordering hash");
        assert_eq!(h1, h2);
    }

    #[test]
    fn ordering_hash_matches_expected_digest() {
        let mut batch =
            TransitionBatch::new("fastpq-lane-balanced", crate::PublicInputs::default());
        batch.push(StateTransition::new(
            b"k1".to_vec(),
            vec![0x01],
            vec![0x02],
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            b"k2".to_vec(),
            vec![0x03],
            vec![0x04],
            OperationKind::Mint,
        ));
        batch.sort();

        let hash = ordering_hash(&batch).expect("ordering hash");
        let raw: [u8; iroha_crypto::Hash::LENGTH] = hash.into();
        assert_eq!(
            raw,
            [
                0x6A, 0xF7, 0xE7, 0x61, 0xFA, 0x29, 0x60, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x01,
            ]
        );
    }

    #[test]
    fn ordering_hash_distinguishes_trailing_zero_bytes() {
        let mut baseline =
            TransitionBatch::new("fastpq-lane-balanced", crate::PublicInputs::default());
        baseline.push(StateTransition::new(
            b"key".to_vec(),
            vec![0x01],
            vec![],
            OperationKind::Transfer,
        ));

        let mut padded =
            TransitionBatch::new("fastpq-lane-balanced", crate::PublicInputs::default());
        padded.push(StateTransition::new(
            b"key".to_vec(),
            vec![0x01, 0x00],
            vec![],
            OperationKind::Transfer,
        ));

        let h_baseline = ordering_hash(&baseline).expect("ordering hash");
        let h_padded = ordering_hash(&padded).expect("ordering hash");

        assert_ne!(h_baseline, h_padded);
    }
}
