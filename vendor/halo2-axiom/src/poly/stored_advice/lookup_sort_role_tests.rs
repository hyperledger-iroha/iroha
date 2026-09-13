//! Authenticated sorted-run metadata, with no claim of lookup membership or backend security.

use super::*;

fn sorted_layout(field: StoredPastaFieldV1, k: u32, run_log: u32) -> StoredPolynomialLayoutV1 {
    StoredPolynomialLayoutV1::new(
        [71; 32],
        17,
        field,
        StoredPolynomialBasisV1::Lagrange,
        k,
        StoredPolynomialRoleV1::LookupSorted {
            lookup: 3,
            side: StoredLookupSideV1::Input,
            run_log,
        },
    )
    .unwrap()
}

#[test]
fn sorted_run_binding_separates_every_pass_side_lookup_and_original_role() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut bindings = std::collections::BTreeSet::new();
        for lookup in [0, 3, u32::MAX] {
            for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
                let mut layout = sorted_layout(field, 10, 8);
                layout.role = StoredPolynomialRoleV1::LookupCompressed { lookup, side };
                assert!(bindings.insert(layout.context_digest()));
                for run_log in 8..=10 {
                    layout.role = StoredPolynomialRoleV1::LookupSorted {
                        lookup,
                        side,
                        run_log,
                    };
                    assert!(bindings.insert(layout.context_digest()));
                    assert_eq!(
                        layout.advice_coordinates(),
                        Err(StoredPolynomialErrorV1::Context)
                    );
                }
            }
            let mut layout = sorted_layout(field, 10, 8);
            layout.role = StoredPolynomialRoleV1::Advice {
                column: lookup,
                phase: 0,
            };
            assert!(bindings.insert(layout.context_digest()));
        }
        assert_eq!(bindings.len(), 27);
        assert!(!bindings.contains(&[0; 32]));
    }
}

#[test]
fn sorted_run_binding_retains_context_ordinal_field_domain_and_chunk_geometry() {
    let original = sorted_layout(StoredPastaFieldV1::Fp, 10, 8);
    let mut variants = vec![original];
    let mut changed = original;
    changed.proof_context[0] ^= 1;
    variants.push(changed);
    let mut changed = original;
    changed.ordinal += 1;
    variants.push(changed);
    let mut changed = original;
    changed.field = StoredPastaFieldV1::Fq;
    variants.push(changed);
    let mut changed = original;
    changed.k = 9;
    variants.push(changed);
    let digests = variants
        .iter()
        .map(|value| value.context_digest())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(digests.len(), variants.len());
    assert!(!original.same_proof_context(variants[1]));
    assert!(original.same_proof_context(variants[2]));
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for k in 0..=STORED_MAX_K_V1 {
            for run_log in k.min(8)..=k {
                let layout = sorted_layout(field, k, run_log);
                assert_eq!(layout.scalar_count(), 1 << k);
                let counts = (0..layout.chunk_count())
                    .map(|chunk| layout.chunk_scalar_count(chunk as u64).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(counts.iter().sum::<usize>(), 1 << k);
                assert!(counts.iter().all(|count| (1..=256).contains(count)));
                assert_eq!(
                    layout.chunk_scalar_count(layout.chunk_count() as u64),
                    Err(StoredPolynomialErrorV1::ChunkIndex)
                );
            }
        }
    }
}

#[test]
fn sorted_run_layout_rejects_non_lagrange_and_impossible_pass_geometry() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for k in [0, 4, 8, 9, STORED_MAX_K_V1] {
            let make = |basis, run_log| {
                StoredPolynomialLayoutV1::new(
                    [71; 32],
                    17,
                    field,
                    basis,
                    k,
                    StoredPolynomialRoleV1::LookupSorted {
                        lookup: 3,
                        side: StoredLookupSideV1::Table,
                        run_log,
                    },
                )
            };
            assert!(make(StoredPolynomialBasisV1::Lagrange, k.min(8)).is_ok());
            assert!(make(StoredPolynomialBasisV1::Lagrange, k).is_ok());
            for run_log in [k + 1, u32::MAX] {
                assert_eq!(
                    make(StoredPolynomialBasisV1::Lagrange, run_log),
                    Err(StoredPolynomialErrorV1::Layout)
                );
            }
            if k > 0 {
                assert_eq!(
                    make(StoredPolynomialBasisV1::Lagrange, k.min(8) - 1),
                    Err(StoredPolynomialErrorV1::Layout)
                );
            }
            for basis in [
                StoredPolynomialBasisV1::Coefficient,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 1,
                    part: 0,
                },
            ] {
                assert_eq!(make(basis, k.min(8)), Err(StoredPolynomialErrorV1::Layout));
            }
        }
    }
}
