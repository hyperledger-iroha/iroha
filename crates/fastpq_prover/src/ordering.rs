//! Deterministic ordering helpers for FASTPQ batches.
//!
//! Ordering commitments use the full-width canonical Iroha hash over canonical Norito bytes.
use crate::{Result, TransitionBatch};
use iroha_crypto::Hash;
use norito::core;
/// Domain separation tag for ordering commitments.
const ORDERING_DOMAIN: &[u8] = b"fastpq:v1:ordering";
/// Compute the canonical ordering commitment for a batch.
///
/// # Errors
///
/// Propagates Norito serialization failures.
pub fn ordering_hash(batch: &TransitionBatch) -> Result<Hash> {
    let canonical = batch.canonicalized();
    let encoded = core::to_bytes(&canonical.transitions)?;
    Ok(Hash::new_from_chunks(&[ORDERING_DOMAIN, &encoded]))
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
            OperationKind::MetaSet,
        ));
        original.push(StateTransition::new(
            b"asset/b".to_vec(),
            vec![5],
            vec![6],
            OperationKind::Transfer,
        ));
        let mut permuted = original.clone();
        permuted.transitions.swap(0, 2);
        permuted.transitions.swap(0, 1);
        let h1 = ordering_hash(&original).expect("ordering hash");
        let h2 = ordering_hash(&permuted).expect("ordering hash");
        assert_eq!(h1, h2);
    }
    #[test]
    fn ordering_hash_uses_the_full_domain_separated_digest() {
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
            OperationKind::MetaSet,
        ));
        batch.sort();
        let hash = ordering_hash(&batch).expect("ordering hash");
        let encoded = core::to_bytes(&batch.transitions).expect("encode transitions");
        assert_eq!(hash, Hash::new_from_chunks(&[ORDERING_DOMAIN, &encoded]));
        let raw: [u8; iroha_crypto::Hash::LENGTH] = hash.into();
        assert!(raw[8..].iter().any(|&byte| byte != 0));
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
