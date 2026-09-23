//! Original key-role identity, basis bounds and independently pinned digest framing.
//!
//! Layouts are metadata, not evidence that scalars came from an authenticated original key.

use super::*;
use std::collections::BTreeSet;

fn key_roles() -> Vec<StoredPolynomialRoleV1> {
    use StoredPolynomialRoleV1::{KeyFixed, KeyMask, KeyPermutation};
    let mut roles = Vec::new();
    for column in [0, 3, u32::MAX] {
        roles.extend([KeyFixed { column }, KeyPermutation { column }]);
    }
    for kind in [
        StoredKeyMaskV1::L0,
        StoredKeyMaskV1::LLast,
        StoredKeyMaskV1::LActiveRow,
    ] {
        roles.push(KeyMask { kind });
    }
    roles
}

fn key_layout(
    field: StoredPastaFieldV1,
    basis: StoredPolynomialBasisV1,
    k: u32,
    role: StoredPolynomialRoleV1,
) -> Result<StoredPolynomialLayoutV1, StoredPolynomialErrorV1> {
    StoredPolynomialLayoutV1::new([71; 32], 17, field, basis, k, role)
}

#[test]
fn both_fields_key_roles_preserve_identity_and_exact_geometry_in_each_allowed_basis() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for role in key_roles() {
            for k in 0..=STORED_MAX_K_V1 {
                let mut bases = vec![Coefficient];
                if !matches!(role, StoredPolynomialRoleV1::KeyMask { .. }) {
                    bases.push(Lagrange);
                }
                if k < STORED_MAX_K_V1 {
                    let extension_log = STORED_MAX_K_V1 - k;
                    bases.extend([
                        CosetPart {
                            extension_log,
                            part: 0,
                        },
                        CosetPart {
                            extension_log,
                            part: (1 << extension_log) - 1,
                        },
                    ]);
                }
                for basis in bases {
                    let layout = key_layout(field, basis, k, role).unwrap();
                    assert_eq!(layout.role(), role);
                    assert_eq!(layout.field(), field);
                    assert_eq!(layout.basis(), basis);
                    assert_eq!(layout.k(), k);
                    assert_eq!(layout.ordinal(), 17);
                    assert_eq!(layout.scalar_count(), 1_usize << k);
                    assert_eq!(layout.chunk_count(), (1_usize << k).div_ceil(256));
                    assert_eq!(
                        layout.advice_coordinates(),
                        Err(StoredPolynomialErrorV1::Context)
                    );
                    let last = layout.chunk_count() as u64 - 1;
                    assert_eq!(
                        layout.chunk_scalar_count(last).unwrap(),
                        (1_usize << k).min(256)
                    );
                    assert_eq!(
                        layout.chunk_scalar_count(last + 1),
                        Err(StoredPolynomialErrorV1::ChunkIndex)
                    );
                }
            }
        }
    }
}

#[test]
fn both_fields_key_masks_reject_lagrange_and_all_key_roles_reject_invalid_geometry() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for role in key_roles() {
            for k in [STORED_MAX_K_V1 + 1, u32::MAX] {
                assert_eq!(
                    key_layout(field, Coefficient, k, role),
                    Err(StoredPolynomialErrorV1::Layout)
                );
            }
            assert_eq!(
                StoredPolynomialLayoutV1::new([0; 32], 17, field, Coefficient, 9, role),
                Err(StoredPolynomialErrorV1::Layout)
            );
            for (k, extension_log, part) in [
                (0, 0, 0),
                (0, 1, 2),
                (0, 20, 0),
                (9, 11, 0),
                (18, 1, 2),
                (19, 1, 0),
                (0, u32::MAX, 0),
                (0, 1, u32::MAX),
            ] {
                assert_eq!(
                    key_layout(
                        field,
                        CosetPart {
                            extension_log,
                            part
                        },
                        k,
                        role
                    ),
                    Err(StoredPolynomialErrorV1::Layout)
                );
            }
            if matches!(role, StoredPolynomialRoleV1::KeyMask { .. }) {
                for k in 0..=STORED_MAX_K_V1 {
                    assert_eq!(
                        key_layout(field, Lagrange, k, role),
                        Err(StoredPolynomialErrorV1::Layout)
                    );
                }
            }
        }
    }
}

#[test]
fn key_role_bindings_separate_every_mask_column_basis_field_and_existing_polynomial_role() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    use StoredPolynomialRoleV1 as Role;
    let mut roles = key_roles();
    roles.extend([
        Role::Advice {
            column: 3,
            phase: 0,
        },
        Role::Advice {
            column: 3,
            phase: 1,
        },
        Role::LookupCompressed {
            lookup: 3,
            side: StoredLookupSideV1::Input,
        },
        Role::LookupCompressed {
            lookup: 3,
            side: StoredLookupSideV1::Table,
        },
        Role::LookupSorted {
            lookup: 3,
            side: StoredLookupSideV1::Input,
            run_log: 8,
        },
        Role::LookupLeftoverTable { lookup: 3 },
        Role::LookupPermuted {
            lookup: 3,
            side: StoredLookupSideV1::Input,
        },
        Role::LookupPermuted {
            lookup: 3,
            side: StoredLookupSideV1::Table,
        },
        Role::CopyPermutationProduct { set: 3 },
        Role::LookupProduct { lookup: 3 },
        Role::Instance { column: 3 },
        Role::VanishingRandom,
        Role::QuotientNumerator,
        Role::QuotientAliasedPart {
            part: 1,
            extension_log: 2,
        },
        Role::QuotientPiece { piece: 1 },
    ]);
    let mut digests = BTreeSet::new();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for role in roles.iter().copied() {
            for basis in [
                Lagrange,
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
                    extension_log: 3,
                    part: 1,
                },
            ] {
                let Ok(original) = key_layout(field, basis, 9, role) else {
                    continue;
                };
                let mut variants = [original; 4];
                variants[1].proof_context[0] ^= 1;
                variants[2].ordinal += 1;
                variants[3].k += 1;
                for layout in variants {
                    assert!(
                        digests.insert(layout.context_digest()),
                        "aliased binding: {layout:?}"
                    );
                    assert_ne!(layout.context_digest(), [0; 32]);
                    assert_eq!(layout.role(), role);
                }
            }
        }
    }
}

#[test]
fn key_role_tags_12_through_14_and_unchanged_tags_0_through_11_match_independent_vectors() {
    use StoredPolynomialRoleV1 as Role;
    let vectors = [
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            Role::Advice {
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
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupCompressed {
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
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupSorted {
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
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupLeftoverTable { lookup: 3 },
            [
                116, 9, 207, 163, 238, 94, 188, 232, 137, 214, 249, 198, 232, 104, 201, 153, 115,
                126, 176, 49, 6, 243, 12, 237, 149, 19, 40, 174, 119, 225, 180, 16,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupPermuted {
                lookup: 3,
                side: StoredLookupSideV1::Table,
            },
            [
                73, 48, 223, 89, 229, 132, 187, 9, 253, 85, 197, 138, 143, 93, 203, 176, 167, 168,
                197, 100, 48, 112, 241, 17, 12, 196, 9, 147, 57, 33, 24, 211,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            Role::CopyPermutationProduct { set: 3 },
            [
                140, 72, 96, 62, 141, 52, 111, 95, 89, 167, 216, 168, 147, 35, 5, 172, 216, 227,
                165, 224, 17, 227, 97, 157, 80, 169, 26, 143, 7, 169, 133, 55,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupProduct { lookup: 3 },
            [
                226, 186, 147, 122, 34, 69, 235, 155, 34, 37, 21, 230, 65, 27, 138, 255, 125, 205,
                59, 159, 95, 234, 113, 52, 97, 150, 226, 181, 155, 160, 165, 39,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            Role::Instance { column: 3 },
            [
                39, 169, 10, 123, 167, 170, 74, 28, 43, 190, 15, 145, 65, 52, 26, 253, 233, 243,
                224, 251, 252, 69, 47, 213, 122, 22, 48, 42, 4, 204, 166, 5,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::VanishingRandom,
            [
                41, 115, 86, 32, 121, 85, 219, 233, 48, 141, 51, 222, 197, 222, 89, 175, 110, 7,
                37, 129, 210, 111, 36, 109, 13, 179, 156, 76, 120, 93, 121, 253,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 2,
                part: 1,
            },
            Role::QuotientNumerator,
            [
                247, 181, 196, 231, 197, 242, 54, 161, 25, 131, 122, 22, 101, 156, 94, 106, 47,
                202, 32, 45, 206, 199, 130, 239, 133, 119, 25, 145, 86, 87, 31, 159,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::QuotientAliasedPart {
                part: 1,
                extension_log: 2,
            },
            [
                138, 116, 155, 105, 184, 216, 112, 230, 67, 148, 209, 21, 50, 90, 142, 127, 244,
                235, 170, 11, 158, 91, 28, 106, 9, 2, 195, 208, 103, 42, 7, 241,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::QuotientPiece { piece: 1 },
            [
                239, 194, 90, 174, 17, 194, 112, 104, 234, 128, 157, 169, 12, 221, 195, 179, 25,
                184, 149, 92, 192, 29, 183, 126, 137, 109, 179, 216, 174, 209, 250, 188,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyFixed { column: 3 },
            [
                174, 137, 105, 110, 88, 168, 230, 6, 86, 213, 58, 46, 135, 30, 141, 59, 100, 1, 8,
                24, 253, 245, 43, 61, 108, 182, 141, 212, 185, 212, 43, 237,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyPermutation { column: 3 },
            [
                228, 198, 192, 229, 107, 161, 61, 209, 195, 84, 193, 114, 4, 25, 147, 1, 210, 171,
                25, 1, 123, 168, 3, 237, 237, 141, 14, 36, 215, 138, 6, 104,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyMask {
                kind: StoredKeyMaskV1::L0,
            },
            [
                173, 173, 192, 109, 109, 184, 136, 102, 189, 96, 245, 22, 4, 4, 138, 54, 153, 243,
                157, 29, 177, 188, 46, 112, 241, 201, 100, 147, 221, 66, 197, 83,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyMask {
                kind: StoredKeyMaskV1::LLast,
            },
            [
                215, 228, 131, 81, 191, 207, 132, 10, 109, 224, 127, 72, 124, 150, 47, 83, 204, 57,
                255, 54, 88, 15, 51, 74, 17, 95, 103, 28, 222, 250, 163, 241,
            ],
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyMask {
                kind: StoredKeyMaskV1::LActiveRow,
            },
            [
                82, 250, 233, 54, 185, 77, 176, 127, 124, 139, 200, 168, 239, 201, 94, 232, 178,
                201, 196, 28, 192, 80, 220, 167, 102, 135, 151, 142, 18, 74, 39, 3,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            Role::Advice {
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
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupCompressed {
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
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupSorted {
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
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupLeftoverTable { lookup: 3 },
            [
                204, 44, 119, 201, 178, 186, 94, 126, 154, 11, 58, 27, 226, 255, 140, 110, 99, 110,
                94, 163, 133, 98, 57, 158, 221, 246, 123, 107, 36, 105, 117, 164,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupPermuted {
                lookup: 3,
                side: StoredLookupSideV1::Table,
            },
            [
                174, 67, 47, 211, 45, 43, 251, 83, 173, 0, 231, 228, 41, 52, 184, 175, 93, 224,
                126, 13, 242, 183, 123, 78, 26, 113, 164, 164, 229, 130, 8, 104,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            Role::CopyPermutationProduct { set: 3 },
            [
                70, 90, 204, 244, 252, 51, 116, 80, 130, 225, 21, 212, 43, 16, 138, 248, 63, 142,
                8, 175, 127, 146, 14, 158, 255, 70, 230, 181, 189, 56, 133, 174,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            Role::LookupProduct { lookup: 3 },
            [
                0, 0, 43, 198, 30, 75, 84, 227, 136, 214, 70, 118, 76, 124, 53, 194, 181, 175, 100,
                38, 61, 4, 173, 113, 228, 113, 50, 177, 45, 53, 71, 65,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            Role::Instance { column: 3 },
            [
                50, 11, 212, 110, 53, 109, 160, 128, 52, 103, 132, 51, 197, 6, 208, 24, 179, 95,
                163, 241, 126, 68, 104, 198, 16, 193, 196, 107, 59, 172, 12, 159,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::VanishingRandom,
            [
                220, 237, 10, 190, 114, 158, 146, 91, 58, 132, 119, 148, 166, 24, 40, 240, 231,
                229, 238, 165, 91, 96, 18, 80, 34, 131, 193, 96, 185, 143, 123, 247,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 2,
                part: 1,
            },
            Role::QuotientNumerator,
            [
                189, 64, 151, 34, 197, 159, 202, 203, 199, 254, 68, 100, 240, 226, 74, 1, 242, 84,
                206, 232, 21, 215, 83, 214, 248, 52, 247, 97, 125, 184, 241, 77,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::QuotientAliasedPart {
                part: 1,
                extension_log: 2,
            },
            [
                163, 250, 209, 120, 199, 109, 241, 70, 103, 75, 30, 138, 190, 33, 36, 205, 139,
                178, 15, 60, 101, 8, 23, 56, 60, 181, 33, 236, 108, 40, 124, 79,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::QuotientPiece { piece: 1 },
            [
                138, 13, 39, 223, 235, 147, 150, 14, 185, 36, 106, 62, 55, 90, 230, 128, 178, 133,
                130, 251, 61, 238, 173, 116, 242, 181, 80, 63, 176, 191, 250, 109,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyFixed { column: 3 },
            [
                216, 73, 8, 224, 124, 178, 228, 11, 96, 233, 93, 174, 7, 0, 229, 82, 37, 195, 233,
                76, 222, 4, 101, 160, 47, 129, 64, 239, 180, 66, 88, 37,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyPermutation { column: 3 },
            [
                34, 233, 113, 244, 7, 50, 168, 59, 89, 215, 198, 95, 246, 114, 228, 233, 232, 62,
                252, 112, 97, 142, 192, 247, 54, 76, 30, 61, 130, 50, 189, 230,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyMask {
                kind: StoredKeyMaskV1::L0,
            },
            [
                86, 155, 170, 247, 141, 75, 245, 217, 115, 233, 132, 218, 230, 137, 228, 156, 157,
                215, 176, 216, 133, 108, 44, 88, 135, 15, 135, 245, 207, 36, 33, 71,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyMask {
                kind: StoredKeyMaskV1::LLast,
            },
            [
                235, 248, 100, 169, 90, 55, 172, 195, 128, 234, 127, 97, 136, 231, 222, 179, 173,
                97, 203, 102, 222, 22, 203, 215, 149, 150, 94, 171, 25, 35, 54, 254,
            ],
        ),
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Coefficient,
            Role::KeyMask {
                kind: StoredKeyMaskV1::LActiveRow,
            },
            [
                4, 175, 49, 89, 28, 227, 246, 243, 241, 186, 88, 39, 168, 186, 176, 122, 57, 22,
                26, 63, 166, 145, 131, 205, 31, 221, 140, 208, 56, 25, 240, 147,
            ],
        ),
    ];
    for (field, basis, role, expected) in vectors {
        assert_eq!(
            key_layout(field, basis, 10, role).unwrap().context_digest(),
            expected
        );
    }
}
