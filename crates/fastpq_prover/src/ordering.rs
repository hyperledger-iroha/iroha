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
/// The committed Norito layout is fixed independently of any enclosing decode
/// context, so identical transitions have one ordering hash on every caller.
///
/// # Errors
///
/// Propagates Norito serialization failures.
pub fn ordering_hash(batch: &TransitionBatch) -> Result<Hash> {
    let canonical = batch.canonicalized();
    let encoded = {
        let _canonical = core::DecodeFlagsGuard::enter(core::default_encode_flags());
        core::to_bytes(&canonical.transitions)?
    };
    Ok(Hash::new_from_chunks(&[ORDERING_DOMAIN, &encoded]))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OperationKind, StateTransition};
    #[test]
    fn ordering_hash_stable_under_permutations() {
        let mut original = TransitionBatch::new(
            "fastpq-state-transition-stark-v1",
            crate::PublicInputs::default(),
        );
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
            OperationKind::MetaSet,
        ));
        let mut permuted = original.clone();
        permuted.transitions.swap(0, 2);
        permuted.transitions.swap(0, 1);
        let h1 = ordering_hash(&original).expect("ordering hash");
        let h2 = ordering_hash(&permuted).expect("ordering hash");
        assert_eq!(h1, h2);
    }
    #[test]
    fn ordering_hash_is_independent_of_ambient_norito_layout() {
        let mut batch = TransitionBatch::new(
            "fastpq-state-transition-stark-v1",
            crate::PublicInputs::default(),
        );
        batch.push(StateTransition::new(
            b"metadata/layout".to_vec(),
            vec![1, 2, 3],
            vec![4, 5, 6],
            OperationKind::MetaSet,
        ));
        let expected = ordering_hash(&batch).expect("canonical ordering commitment");
        let canonical_bytes = core::to_bytes(&batch.transitions).expect("canonical encoding");
        let alternate_flags = core::default_encode_flags() ^ core::header_flags::COMPACT_LEN;
        let _ambient = core::DecodeFlagsGuard::enter(alternate_flags);
        let alternate_bytes = core::to_bytes(&batch.transitions).expect("alternate encoding");
        assert_ne!(
            canonical_bytes, alternate_bytes,
            "fixture must exercise another layout"
        );
        assert_eq!(
            ordering_hash(&batch).expect("ordering commitment in another decode context"),
            expected
        );
        assert_eq!(core::effective_decode_flags(), Some(alternate_flags));
    }
    #[test]
    fn ordering_hash_uses_the_full_domain_separated_digest() {
        let mut batch = TransitionBatch::new(
            "fastpq-state-transition-stark-v1",
            crate::PublicInputs::default(),
        );
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
        let mut baseline = TransitionBatch::new(
            "fastpq-state-transition-stark-v1",
            crate::PublicInputs::default(),
        );
        baseline.push(StateTransition::new(
            b"key".to_vec(),
            vec![0x01],
            vec![],
            OperationKind::Transfer,
        ));
        let mut padded = TransitionBatch::new(
            "fastpq-state-transition-stark-v1",
            crate::PublicInputs::default(),
        );
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
    #[test]
    fn ordering_hash_binds_permission_payload_and_epoch() {
        let permission_transition = |permission_id, epoch| {
            let mut batch = TransitionBatch::new(
                "fastpq-state-transition-stark-v1",
                crate::PublicInputs::default(),
            );
            batch.push(StateTransition::new(
                b"permission/key".to_vec(),
                Vec::new(),
                vec![1],
                OperationKind::RoleGrant {
                    role_id: [0x11; 32],
                    permission_id,
                    epoch,
                },
            ));
            batch
        };
        let baseline = permission_transition([0x22; 32], 7);
        let changed_permission = permission_transition([0x23; 32], 7);
        let changed_epoch = permission_transition([0x22; 32], 8);
        assert_ne!(
            ordering_hash(&baseline).expect("baseline ordering hash"),
            ordering_hash(&changed_permission).expect("permission ordering hash")
        );
        assert_ne!(
            ordering_hash(&baseline).expect("baseline ordering hash"),
            ordering_hash(&changed_epoch).expect("epoch ordering hash")
        );
    }
}
