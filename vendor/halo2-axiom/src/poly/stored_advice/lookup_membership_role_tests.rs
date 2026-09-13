//! Authenticated leftover metadata and fixed wire-binding regression vectors.

use super::*;

fn leftover_layout(field: StoredPastaFieldV1, k: u32, lookup: u32) -> StoredPolynomialLayoutV1 {
    StoredPolynomialLayoutV1::new(
        [71; 32],
        17,
        field,
        StoredPolynomialBasisV1::Lagrange,
        k,
        StoredPolynomialRoleV1::LookupLeftoverTable { lookup },
    )
    .unwrap()
}

#[test]
fn leftover_role_tag3_and_existing_tags0_through2_match_independent_digest_vectors() {
    // Generated independently with Python hashlib.blake2b from the documented little-endian
    // binding bytes and personalization, not by the Rust layout implementation.
    let vectors = [
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::Advice {
                column: 3,
                phase: 0,
            },
            [
                219, 123, 226, 250, 210, 158, 52, 228, 198, 43, 25, 59, 61, 21, 119, 192, 141, 232,
                69, 33, 117, 93, 85, 15, 106, 64, 207, 196, 185, 9, 93, 192,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 3,
                side: StoredLookupSideV1::Input,
            },
            [
                181, 30, 139, 182, 220, 102, 126, 11, 14, 37, 11, 122, 49, 211, 122, 150, 225, 229,
                71, 22, 180, 234, 137, 60, 158, 32, 61, 158, 43, 173, 100, 135,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 3,
                side: StoredLookupSideV1::Table,
            },
            [
                140, 59, 195, 96, 255, 209, 150, 160, 147, 12, 27, 78, 171, 200, 245, 8, 222, 255,
                221, 103, 78, 62, 175, 149, 208, 25, 224, 143, 186, 229, 96, 233,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side: StoredLookupSideV1::Input,
                run_log: 8,
            },
            [
                77, 173, 96, 243, 207, 186, 188, 146, 202, 217, 203, 173, 152, 132, 46, 97, 97, 77,
                196, 144, 155, 68, 52, 191, 220, 155, 222, 103, 86, 40, 112, 68,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side: StoredLookupSideV1::Table,
                run_log: 8,
            },
            [
                236, 254, 173, 128, 231, 169, 151, 79, 163, 243, 220, 148, 134, 98, 165, 73, 123,
                225, 143, 97, 153, 238, 153, 72, 230, 21, 98, 184, 202, 179, 79, 177,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side: StoredLookupSideV1::Input,
                run_log: 10,
            },
            [
                193, 157, 33, 199, 203, 246, 218, 178, 61, 213, 155, 208, 71, 6, 160, 220, 114,
                170, 4, 155, 186, 18, 199, 20, 255, 206, 225, 171, 42, 182, 127, 21,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialRoleV1::LookupLeftoverTable { lookup: 3 },
            [
                116, 9, 207, 163, 238, 94, 188, 232, 137, 214, 249, 198, 232, 104, 201, 153, 115,
                126, 176, 49, 6, 243, 12, 237, 149, 19, 40, 174, 119, 225, 180, 16,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::Advice {
                column: 3,
                phase: 0,
            },
            [
                152, 223, 177, 64, 200, 248, 75, 69, 167, 36, 136, 28, 224, 164, 18, 180, 240, 100,
                215, 69, 102, 169, 165, 86, 154, 83, 74, 240, 39, 114, 170, 19,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 3,
                side: StoredLookupSideV1::Input,
            },
            [
                252, 8, 51, 32, 243, 63, 14, 164, 188, 60, 231, 230, 60, 92, 105, 45, 126, 23, 69,
                250, 219, 104, 186, 36, 216, 233, 50, 92, 156, 253, 14, 117,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 3,
                side: StoredLookupSideV1::Table,
            },
            [
                161, 177, 66, 177, 251, 147, 41, 136, 45, 109, 7, 127, 97, 175, 199, 248, 216, 195,
                102, 114, 115, 40, 51, 223, 74, 177, 119, 231, 60, 231, 158, 145,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side: StoredLookupSideV1::Input,
                run_log: 8,
            },
            [
                212, 106, 212, 219, 44, 238, 252, 181, 226, 2, 233, 74, 195, 161, 135, 109, 141,
                207, 89, 125, 81, 48, 142, 187, 249, 91, 224, 45, 88, 62, 39, 223,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side: StoredLookupSideV1::Table,
                run_log: 8,
            },
            [
                87, 47, 213, 197, 57, 208, 35, 237, 251, 179, 229, 47, 57, 113, 59, 125, 229, 249,
                144, 155, 108, 227, 223, 196, 16, 120, 91, 94, 150, 38, 85, 28,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side: StoredLookupSideV1::Input,
                run_log: 10,
            },
            [
                163, 7, 38, 83, 139, 4, 247, 95, 107, 63, 173, 229, 193, 78, 247, 171, 42, 159, 58,
                133, 123, 135, 104, 79, 73, 195, 45, 41, 9, 63, 145, 47,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialRoleV1::LookupLeftoverTable { lookup: 3 },
            [
                204, 44, 119, 201, 178, 186, 94, 126, 154, 11, 58, 27, 226, 255, 140, 110, 99, 110,
                94, 163, 133, 98, 57, 158, 221, 246, 123, 107, 36, 105, 117, 164,
            ],
        ),
    ];
    let mut unique = std::collections::BTreeSet::new();
    for (field, role, expected) in vectors {
        let layout = StoredPolynomialLayoutV1::new(
            [71; 32],
            17,
            field,
            StoredPolynomialBasisV1::Lagrange,
            10,
            role,
        )
        .unwrap();
        assert_eq!(layout.context_digest(), expected);
        assert!(unique.insert(expected));
    }
    assert_eq!(unique.len(), 14);
}

#[test]
fn leftover_binding_distinguishes_every_context_field_ordinal_domain_lookup_and_prior_role() {
    let original = leftover_layout(StoredPastaFieldV1::Fp, 10, 3);
    let mut variants = vec![original];
    let mut next = original;
    next.proof_context[0] ^= 1;
    variants.push(next);
    let mut next = original;
    next.ordinal += 1;
    variants.push(next);
    let mut next = original;
    next.field = StoredPastaFieldV1::Fq;
    variants.push(next);
    let mut next = original;
    next.k = 9;
    variants.push(next);
    for lookup in [0, 4, u32::MAX] {
        let mut next = original;
        next.role = StoredPolynomialRoleV1::LookupLeftoverTable { lookup };
        variants.push(next);
    }
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        let mut next = original;
        next.role = StoredPolynomialRoleV1::LookupCompressed { lookup: 3, side };
        variants.push(next);
        for run_log in 8..=10 {
            let mut next = original;
            next.role = StoredPolynomialRoleV1::LookupSorted {
                lookup: 3,
                side,
                run_log,
            };
            variants.push(next);
        }
    }
    let mut next = original;
    next.role = StoredPolynomialRoleV1::Advice {
        column: 3,
        phase: 0,
    };
    variants.push(next);
    let unique = variants
        .iter()
        .map(|layout| layout.context_digest())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), variants.len());
    assert!(!unique.contains(&[0; 32]));
    assert!(!original.same_proof_context(variants[1]));
    assert!(original.same_proof_context(variants[2]));
    assert_eq!(
        original.advice_coordinates(),
        Err(StoredPolynomialErrorV1::Context)
    );
}

#[test]
fn leftover_layout_accepts_only_lagrange_and_preserves_all_domain_chunk_bounds() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for k in 0..=STORED_MAX_K_V1 {
            let layout = leftover_layout(field, k, u32::MAX);
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
            for basis in [
                StoredPolynomialBasisV1::Coefficient,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 1,
                    part: 0,
                },
            ] {
                assert_eq!(
                    StoredPolynomialLayoutV1::new(
                        [71; 32],
                        17,
                        field,
                        basis,
                        k,
                        StoredPolynomialRoleV1::LookupLeftoverTable { lookup: 3 }
                    ),
                    Err(StoredPolynomialErrorV1::Layout)
                );
            }
        }
        assert_eq!(
            StoredPolynomialLayoutV1::new(
                [71; 32],
                17,
                field,
                StoredPolynomialBasisV1::Lagrange,
                STORED_MAX_K_V1 + 1,
                StoredPolynomialRoleV1::LookupLeftoverTable { lookup: 3 }
            ),
            Err(StoredPolynomialErrorV1::Layout)
        );
    }
}
