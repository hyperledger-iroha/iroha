//! Original completed advice owners reject self-consistent indexed-key role substitutions.
//!
//! These tests reuse genuine seeded phase commitments. They verify admission, absence of
//! later effects, and destruction of all owned snapshots/blinds, not key-file provenance.

use super::*;
use crate::poly::stored_advice::StoredKeyMaskV1;

fn key_roles() -> [StoredPolynomialRoleV1; 5] {
    [
        StoredPolynomialRoleV1::KeyFixed { column: 0 },
        StoredPolynomialRoleV1::KeyPermutation { column: 0 },
        StoredPolynomialRoleV1::KeyMask {
            kind: StoredKeyMaskV1::L0,
        },
        StoredPolynomialRoleV1::KeyMask {
            kind: StoredKeyMaskV1::LLast,
        },
        StoredPolynomialRoleV1::KeyMask {
            kind: StoredKeyMaskV1::LActiveRow,
        },
    ]
}

fn substituted_layout(
    original: StoredPolynomialLayoutV1,
    role: StoredPolynomialRoleV1,
) -> StoredPolynomialLayoutV1 {
    // The requested key identities are valid metadata. Masks have a native coefficient
    // representation; fixed and permutation columns retain the matching Lagrange geometry.
    let basis = if matches!(role, StoredPolynomialRoleV1::KeyMask { .. }) {
        StoredPolynomialBasisV1::Coefficient
    } else {
        original.basis()
    };
    StoredPolynomialLayoutV1::new(
        original.proof_context,
        original.ordinal(),
        original.field(),
        basis,
        original.k(),
        role,
    )
    .unwrap()
}

fn rejects_key_substitution<C>(after_completion: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for role in key_roles() {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (mut committed, mut rng, transcript) = absorbed_through(&params, &backend, 2);
        assert!(committed.is_complete());
        let original = committed.session.columns[0].layout;
        let forged = substituted_layout(original, role);
        let before = {
            let record = backend.record.borrow();
            (
                record.read_count,
                record.rng_draws,
                record.sealed.len(),
                record.writer_drops,
            )
        };
        let transcript_before = (transcript.writes, transcript.squeezes);
        let mut expected_rng = rng.inner.clone();
        if after_completion {
            let mut complete = committed.into_complete().unwrap();
            let column = &mut complete.session.as_mut().unwrap().columns[0];
            // Change both cached and live metadata, so mere equality cannot reject this.
            column.layout = forged;
            column.snapshot.layout = forged;
            let mut called = false;
            let result = complete.with_chunk(forged, 0, |_, _| {
                called = true;
                Ok(())
            });
            assert_eq!(
                result,
                Err(StoredPhaseErrorV1::Store(StoredPolynomialErrorV1::Context))
            );
            assert!(!called);
            assert_poisoned(&mut complete, original);
        } else {
            let column = &mut committed.session.columns[0];
            column.layout = forged;
            column.snapshot.layout = forged;
            assert!(matches!(
                committed.into_complete(),
                Err(StoredPhaseErrorV1::Admission)
            ));
        }
        assert_dropped(&backend, 5);
        let record = backend.record.borrow();
        assert_eq!(
            (
                record.read_count,
                record.rng_draws,
                record.sealed.len(),
                record.writer_drops
            ),
            before,
            "key role reached phase backend: {role:?}",
        );
        drop(record);
        assert_eq!((transcript.writes, transcript.squeezes), transcript_before);
        let mut actual_next = [0; 64];
        let mut expected_next = [0; 64];
        rng.fill_bytes(&mut actual_next);
        expected_rng.fill_bytes(&mut expected_next);
        assert_eq!(actual_next, expected_next);
    }
}

#[test]
fn both_pasta_key_roles_cannot_replace_self_consistent_terminal_advice_receipts() {
    rejects_key_substitution::<EqAffine>(false);
    rejects_key_substitution::<EpAffine>(false);
}

#[test]
fn both_pasta_key_roles_cannot_read_as_advice_after_completion() {
    rejects_key_substitution::<EqAffine>(true);
    rejects_key_substitution::<EpAffine>(true);
}
