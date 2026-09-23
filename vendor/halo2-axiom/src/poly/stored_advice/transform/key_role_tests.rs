//! Key roles retain exact identity through original basis transforms and refuse substitutions.

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

fn mask_conversions<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    for kind in [
        StoredKeyMaskV1::L0,
        StoredKeyMaskV1::LLast,
        StoredKeyMaskV1::LActiveRow,
    ] {
        let role = StoredPolynomialRoleV1::KeyMask { kind };
        for k in [0, 3, 9] {
            let domain = EvaluationDomain::<F>::new(5, k);
            let coefficients = (0..1_u64 << k)
                .map(|i| F::from(i * i + 3 * i + 7))
                .collect::<Vec<_>>();
            let bases = [
                Coefficient,
                CosetPart {
                    extension_log: 2,
                    part: 0,
                },
                CosetPart {
                    extension_log: 2,
                    part: 1,
                },
                CosetPart {
                    extension_log: 2,
                    part: 3,
                },
            ];
            for from in bases {
                let values = oracle(&domain, &coefficients, from);
                for to in bases {
                    let (mut provider, mut source, state) = setup(from, k, &values);
                    source.layout =
                        StoredPolynomialLayoutV1::new([5; 32], 7, F::STORED_FIELD, from, k, role)
                            .unwrap();
                    let expected = source.layout;
                    let destination =
                        convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, to)
                            .unwrap();
                    assert_eq!(
                        destination.values,
                        oracle(&domain, &coefficients, to)
                            .iter()
                            .map(|v| v.to_repr())
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(destination.layout.role(), role);
                    assert_eq!(destination.layout.basis(), to);
                    assert_eq!(destination.layout.ordinal(), expected.ordinal() + 1);
                    assert_eq!(destination.layout.field(), expected.field());
                    assert_eq!(destination.layout.k(), expected.k());
                    assert_eq!(destination.layout.proof_context, expected.proof_context);
                    assert_eq!(source.layout, expected);
                    assert_eq!(
                        source.values,
                        values.iter().map(|v| v.to_repr()).collect::<Vec<_>>()
                    );
                    let record = state.record.borrow();
                    assert_eq!((record.creates, record.seals), (1, 1));
                    assert_eq!(
                        (record.reads, record.writes),
                        (expected.chunk_count(), expected.chunk_count())
                    );
                }
                let (mut provider, mut source, state) = setup(from, k, &values);
                source.layout.role = role;
                let expected = source.layout;
                assert!(matches!(
                    convert_stored_advice_v1(
                        &domain,
                        &mut provider,
                        &mut source,
                        expected,
                        Lagrange
                    ),
                    Err(StoredPolynomialErrorV1::Layout)
                ));
                let record = state.record.borrow();
                assert_eq!(
                    (record.creates, record.reads, record.writes, record.seals),
                    (0, 0, 0, 0)
                );
                assert_eq!(source.layout, expected);
                assert!(!source.poisoned);
                assert!(!state.active.get());
            }
        }
    }
}

#[test]
fn both_fields_key_fixed_and_permutation_all_basis_pairs_preserve_original_identity() {
    for role in [
        StoredPolynomialRoleV1::KeyFixed { column: 3 },
        StoredPolynomialRoleV1::KeyPermutation { column: 3 },
    ] {
        conversion_matrix::<Fp>(role);
        conversion_matrix::<Fq>(role);
    }
}

#[test]
fn both_fields_three_original_key_masks_convert_only_between_coefficients_and_coset_parts() {
    mask_conversions::<Fp>();
    mask_conversions::<Fq>();
}

fn identity_refusal<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart};
    let k = 3;
    let domain = EvaluationDomain::<F>::new(5, k);
    let values = (0..1_u64 << k).map(|i| F::from(i + 1)).collect::<Vec<_>>();
    let to = CosetPart {
        extension_log: 2,
        part: 1,
    };
    for original in key_roles() {
        let mut replacements = key_roles()
            .into_iter()
            .filter(|role| *role != original)
            .collect::<Vec<_>>();
        replacements.extend([
            StoredPolynomialRoleV1::KeyFixed { column: 1 },
            StoredPolynomialRoleV1::KeyPermutation { column: 1 },
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0,
            },
            StoredPolynomialRoleV1::CopyPermutationProduct { set: 0 },
            StoredPolynomialRoleV1::Instance { column: 0 },
        ]);
        for replacement in replacements {
            // A false expected receipt cannot cause provider or source side effects.
            let (mut provider, mut source, state) = setup(Coefficient, k, &values);
            source.layout.role = original;
            let mut expected = source.layout;
            expected.role = replacement;
            assert!(matches!(
                convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, to),
                Err(StoredPolynomialErrorV1::Context)
            ));
            assert_eq!(
                (state.record.borrow().creates, state.record.borrow().reads),
                (0, 0)
            );
            assert_eq!(source.layout.role(), original);
            // A provider's wrong role/index/mask is refused before reading the source.
            let (mut provider, mut source, state) = setup(Coefficient, k, &values);
            source.layout.role = original;
            let expected = source.layout;
            state.record.borrow_mut().destination_override = Some(
                StoredPolynomialLayoutV1::new([5; 32], 8, F::STORED_FIELD, to, k, replacement)
                    .unwrap(),
            );
            assert!(matches!(
                convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, to),
                Err(StoredPolynomialErrorV1::Context)
            ));
            let record = state.record.borrow();
            assert_eq!(
                (
                    record.creates,
                    record.reads,
                    record.writes,
                    record.seals,
                    record.writer_drops
                ),
                (1, 0, 0, 0, 1)
            );
            assert!(!state.active.get());
        }
        // A source or writer changing its role after a successful callback is also refused.
        for drift_source in [false, true] {
            let (mut provider, mut source, state) = setup(Coefficient, k, &values);
            source.layout.role = original;
            let expected = source.layout;
            if drift_source {
                state.record.borrow_mut().change_source_after_read = Some(0);
            } else {
                state.record.borrow_mut().change_writer_after_write = true;
            }
            assert!(matches!(
                convert_stored_advice_v1(&domain, &mut provider, &mut source, expected, to),
                Err(StoredPolynomialErrorV1::Context)
            ));
            assert_eq!(state.record.borrow().seals, 0);
            assert_eq!(state.record.borrow().writer_drops, 1);
            assert!(!state.active.get());
        }
    }
}

#[test]
fn both_fields_key_role_conversions_reject_expected_provider_and_callback_identity_changes() {
    identity_refusal::<Fp>();
    identity_refusal::<Fq>();
}
