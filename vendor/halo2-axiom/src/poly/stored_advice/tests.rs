//! Metadata, canonical encoding and geometry tests for confidential polynomial storage.

use super::*;

fn layout() -> StoredPolynomialLayoutV1 {
    StoredPolynomialLayoutV1::new(
        [7; 32],
        4,
        StoredPastaFieldV1::Fp,
        StoredPolynomialBasisV1::Lagrange,
        16,
        StoredPolynomialRoleV1::Advice {
            column: 9,
            phase: 0,
        },
    )
    .unwrap()
}

#[test]
fn metadata_binding_separates_every_polynomial_coordinate() {
    let original = layout();
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
    changed.k -= 1;
    variants.push(changed);
    let mut changed = original;
    changed.role = StoredPolynomialRoleV1::Advice {
        column: 10,
        phase: 0,
    };
    variants.push(changed);
    let mut changed = original;
    changed.role = StoredPolynomialRoleV1::Advice {
        column: 9,
        phase: 1,
    };
    variants.push(changed);
    let mut changed = original;
    changed.basis = StoredPolynomialBasisV1::Coefficient;
    variants.push(changed);
    for (extension_log, part) in [(1, 0), (2, 0), (2, 1)] {
        let mut changed = original;
        changed.basis = StoredPolynomialBasisV1::CosetPart {
            extension_log,
            part,
        };
        variants.push(changed);
    }
    let bindings = variants
        .iter()
        .map(|value| value.context_digest())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(bindings.len(), variants.len());
    assert!(!bindings.contains(&[0; 32]));
    assert_eq!(original.context_digest(), layout().context_digest());
    assert_eq!(
        (
            original.field(),
            original.basis(),
            original.ordinal(),
            original.k(),
            original.advice_coordinates().unwrap().0,
            original.advice_coordinates().unwrap().1
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            4,
            16,
            9,
            0
        )
    );
}

#[test]
fn layout_rejects_invalid_domains_phases_and_coset_parts() {
    let make = |proof, k, phase, basis| {
        StoredPolynomialLayoutV1::new(
            proof,
            0,
            StoredPastaFieldV1::Fp,
            basis,
            k,
            StoredPolynomialRoleV1::Advice { column: 0, phase },
        )
    };
    let lagrange = StoredPolynomialBasisV1::Lagrange;
    assert_eq!(
        make([0; 32], 16, 0, lagrange),
        Err(StoredPolynomialErrorV1::Layout)
    );
    assert_eq!(
        make([1; 32], 20, 0, lagrange),
        Err(StoredPolynomialErrorV1::Layout)
    );
    assert_eq!(
        make([1; 32], 16, 3, lagrange),
        Err(StoredPolynomialErrorV1::Layout)
    );
    for (extension_log, part) in [(0, 0), (4, 0), (3, 8), (u32::MAX, 0)] {
        assert_eq!(
            make(
                [1; 32],
                16,
                0,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log,
                    part
                }
            ),
            Err(StoredPolynomialErrorV1::Layout)
        );
    }
    assert!(
        make(
            [1; 32],
            16,
            2,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 3,
                part: 7
            }
        )
        .is_ok()
    );
}

#[test]
fn chunk_geometry_is_exact_bounded_and_has_one_canonical_tail() {
    for k in 0..=STORED_MAX_K_V1 {
        let layout = StoredPolynomialLayoutV1::new(
            [1; 32],
            0,
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            k,
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0,
            },
        )
        .unwrap();
        let counts = (0..layout.chunk_count())
            .map(|index| layout.chunk_scalar_count(index as u64).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(counts.iter().sum::<usize>(), 1 << k);
        assert!(counts.iter().all(|count| (1..=256).contains(count)));
        assert_eq!(
            layout.chunk_scalar_count(layout.chunk_count() as u64),
            Err(StoredPolynomialErrorV1::ChunkIndex)
        );
        assert_eq!(
            layout.chunk_scalar_count(u64::MAX),
            Err(StoredPolynomialErrorV1::ChunkIndex)
        );
    }
    assert_eq!(STORED_SCALAR_BYTES_V1 * STORED_SCALARS_PER_CHUNK_V1, 8192);
}

#[test]
fn field_encodings_are_canonical_without_modular_reduction() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        assert!(field.is_canonical(&[0; 32]));
        let mut one = [0; 32];
        one[0] = 1;
        assert!(field.is_canonical(&one));
        assert!(!field.is_canonical(&[0xff; 32]));
        let mut high = [0; 32];
        high[31] = 0x80;
        assert!(!field.is_canonical(&high));
    }
}

#[test]
fn role_binding_separates_advice_and_lookup_indices_and_sides() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut bindings = std::collections::BTreeSet::new();
        for index in [0, 1, u32::MAX] {
            for role in [
                StoredPolynomialRoleV1::Advice {
                    column: index,
                    phase: 0,
                },
                StoredPolynomialRoleV1::Advice {
                    column: index,
                    phase: 1,
                },
                StoredPolynomialRoleV1::Advice {
                    column: index,
                    phase: 2,
                },
                StoredPolynomialRoleV1::LookupCompressed {
                    lookup: index,
                    side: StoredLookupSideV1::Input,
                },
                StoredPolynomialRoleV1::LookupCompressed {
                    lookup: index,
                    side: StoredLookupSideV1::Table,
                },
            ] {
                let layout = StoredPolynomialLayoutV1::new(
                    [7; 32],
                    4,
                    field,
                    StoredPolynomialBasisV1::Lagrange,
                    16,
                    role,
                )
                .unwrap();
                assert_eq!(layout.role(), role);
                assert!(
                    bindings.insert(layout.context_digest()),
                    "role binding collided: {role:?}"
                );
            }
        }
        assert_eq!(bindings.len(), 15);
        assert!(!bindings.contains(&[0; 32]));
    }
    // Independent Python hashlib BLAKE2b vectors for the documented fixed-width format.
    // The old advice-only digest must not survive the explicit polynomial domain change.
    let original = layout();
    let old_advice_digest = [
        0x68, 0x3c, 0xf1, 0xf7, 0x38, 0xf6, 0x0d, 0xc6, 0xd3, 0x8d, 0x37, 0xd6, 0xa0, 0x3c, 0x06,
        0x50, 0x89, 0xc4, 0x88, 0x97, 0x16, 0x37, 0x58, 0xdf, 0xd3, 0x71, 0xfe, 0x7e, 0x0e, 0xf4,
        0x16, 0x41,
    ];
    for (role, digest) in [
        (
            StoredPolynomialRoleV1::Advice {
                column: 9,
                phase: 0,
            },
            [
                0x9b, 0xfe, 0x0b, 0xb3, 0xd2, 0x84, 0xec, 0x3c, 0x34, 0x4b, 0xa6, 0x68, 0xd6, 0x6a,
                0x73, 0x75, 0x39, 0x84, 0xff, 0x64, 0xad, 0xfe, 0xd3, 0xbc, 0x1c, 0x39, 0x2b, 0x5a,
                0x97, 0xf6, 0xd7, 0x77,
            ],
        ),
        (
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 9,
                side: StoredLookupSideV1::Input,
            },
            [
                0xd4, 0xa1, 0x89, 0xfd, 0xdb, 0xa8, 0xb3, 0xa1, 0x82, 0x57, 0x0a, 0xcc, 0x0d, 0x35,
                0xce, 0xc8, 0x3a, 0xe0, 0x44, 0x1f, 0x8b, 0xc2, 0xd2, 0x24, 0xd6, 0xee, 0x75, 0x61,
                0x94, 0xb6, 0xfb, 0x37,
            ],
        ),
        (
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 9,
                side: StoredLookupSideV1::Table,
            },
            [
                0x3a, 0x24, 0xe7, 0x40, 0x4d, 0xdb, 0x32, 0xdb, 0x1b, 0x37, 0x67, 0xdc, 0x34, 0xff,
                0x71, 0xc0, 0x7c, 0xaf, 0xd8, 0xdd, 0x30, 0x50, 0x72, 0xb7, 0x52, 0xe6, 0xaf, 0xaf,
                0xd9, 0xf2, 0x01, 0x90,
            ],
        ),
    ] {
        let mut current = original;
        current.role = role;
        assert_eq!(current.context_digest(), digest);
        assert_ne!(current.context_digest(), old_advice_digest);
    }
}

#[test]
fn advice_coordinate_admission_rejects_every_lookup_role() {
    for index in [0, 1, u32::MAX] {
        for phase in 0..=2 {
            let layout = StoredPolynomialLayoutV1::new(
                [7; 32],
                4,
                StoredPastaFieldV1::Fp,
                StoredPolynomialBasisV1::Lagrange,
                0,
                StoredPolynomialRoleV1::Advice {
                    column: index,
                    phase,
                },
            )
            .unwrap();
            assert_eq!(layout.advice_coordinates(), Ok((index, phase)));
        }
        for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
            let layout = StoredPolynomialLayoutV1::new(
                [7; 32],
                4,
                StoredPastaFieldV1::Fp,
                StoredPolynomialBasisV1::Lagrange,
                0,
                StoredPolynomialRoleV1::LookupCompressed {
                    lookup: index,
                    side,
                },
            )
            .unwrap();
            assert_eq!(
                layout.advice_coordinates(),
                Err(StoredPolynomialErrorV1::Context)
            );
        }
    }
}

#[test]
fn lookup_layout_preserves_bounded_geometry_without_advice_phase() {
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
            let role = StoredPolynomialRoleV1::LookupCompressed {
                lookup: u32::MAX,
                side,
            };
            let make = |proof, k, basis| {
                StoredPolynomialLayoutV1::new(proof, u64::MAX, field, basis, k, role)
            };
            for k in 0..=STORED_MAX_K_V1 {
                let layout = make([7; 32], k, StoredPolynomialBasisV1::Lagrange).unwrap();
                assert_eq!(layout.scalar_count(), 1 << k);
                assert_eq!(
                    layout.chunk_count(),
                    (1_usize << k).div_ceil(STORED_SCALARS_PER_CHUNK_V1)
                );
                assert_eq!(
                    layout.chunk_scalar_count((layout.chunk_count() - 1) as u64),
                    Ok((1_usize << k).min(STORED_SCALARS_PER_CHUNK_V1))
                );
                assert_eq!(
                    layout.chunk_scalar_count(layout.chunk_count() as u64),
                    Err(StoredPolynomialErrorV1::ChunkIndex)
                );
                assert_eq!(
                    layout.chunk_scalar_count(u64::MAX),
                    Err(StoredPolynomialErrorV1::ChunkIndex)
                );
                assert_eq!(layout.role(), role);
            }
            assert_eq!(
                make([0; 32], 0, StoredPolynomialBasisV1::Lagrange),
                Err(StoredPolynomialErrorV1::Layout)
            );
            assert_eq!(
                make([7; 32], 20, StoredPolynomialBasisV1::Lagrange),
                Err(StoredPolynomialErrorV1::Layout)
            );
            for (k, extension_log, part) in [
                (19, 1, 0),
                (16, 0, 0),
                (16, 4, 0),
                (16, 3, 8),
                (16, u32::MAX, 0),
            ] {
                assert_eq!(
                    make(
                        [7; 32],
                        k,
                        StoredPolynomialBasisV1::CosetPart {
                            extension_log,
                            part
                        }
                    ),
                    Err(StoredPolynomialErrorV1::Layout)
                );
            }
            let largest = make(
                [7; 32],
                16,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: 3,
                    part: 7,
                },
            )
            .unwrap();
            assert_eq!(largest.role(), role);
            assert_eq!(largest.ordinal(), u64::MAX);
        }
    }
}

#[path = "lookup_sort_role_tests.rs"]
mod lookup_sort_roles;
