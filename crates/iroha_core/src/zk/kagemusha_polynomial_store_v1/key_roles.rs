//! Exact key-role descriptors through the actual encrypted Core spool.
//!
//! These tests authenticate stored metadata, not the provenance of an original proving key.

use super::*;
use halo2_proofs::poly::stored_advice::StoredKeyMaskV1;

fn key_roles() -> [StoredPolynomialRoleV1; 7] {
    [
        StoredPolynomialRoleV1::KeyFixed { column: 0 },
        StoredPolynomialRoleV1::KeyFixed { column: u32::MAX },
        StoredPolynomialRoleV1::KeyPermutation { column: 0 },
        StoredPolynomialRoleV1::KeyPermutation { column: u32::MAX },
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

fn key_snapshot(
    provider: &mut CoreStoredPolynomialProviderV1,
    field: StoredPastaFieldV1,
    basis: StoredPolynomialBasisV1,
    k: u32,
    role: StoredPolynomialRoleV1,
) -> CoreStoredPolynomialSnapshotV1 {
    let mut writer = provider.create(field, basis, k, role).unwrap();
    let layout = writer.layout();
    for chunk in 0..layout.chunk_count() as u64 {
        let values = (0..layout.chunk_scalar_count(chunk).unwrap())
            .map(|row| scalar(chunk * 256 + row as u64 + 1))
            .collect::<Vec<_>>();
        writer.write_chunk(chunk, &values).unwrap();
    }
    writer.seal().unwrap()
}

#[test]
fn key_roles_roundtrip_both_fields_bases_and_chunk_boundaries_with_shared_ordinals() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut previous = None;
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let sentinel = filled(&mut provider, field, 0);
        for role in key_roles() {
            for basis in [
                Lagrange,
                Coefficient,
                CosetPart {
                    extension_log: 1,
                    part: 0,
                },
                CosetPart {
                    extension_log: 1,
                    part: 1,
                },
            ] {
                if matches!(role, StoredPolynomialRoleV1::KeyMask { .. }) && basis == Lagrange {
                    let next = provider.next_ordinal;
                    assert!(matches!(
                        provider.create(field, basis, 1, role),
                        Err(StoredPolynomialErrorV1::Layout)
                    ));
                    assert_eq!(provider.next_ordinal, next);
                    assert_eq!(provider.handles.live.get(), 1);
                    assert!(!provider.window.get());
                    continue;
                }
                for k in [0, 7, 8, 9] {
                    let mut snapshot = key_snapshot(&mut provider, field, basis, k, role);
                    let layout = snapshot.layout();
                    assert_eq!(layout.role(), role);
                    assert_eq!(layout.basis(), basis);
                    assert_eq!(layout.field(), field);
                    assert_eq!(layout.k(), k);
                    assert_eq!(provider.handles.live.get(), 2);
                    if let Some(ordinal) = previous {
                        assert!(layout.ordinal() > ordinal);
                    }
                    previous = Some(layout.ordinal());
                    for chunk in (0..layout.chunk_count() as u64).rev() {
                        snapshot
                            .with_chunk(layout, chunk, |values| {
                                assert_eq!(values.len(), layout.chunk_scalar_count(chunk).unwrap());
                                for (row, value) in values.iter().enumerate() {
                                    assert_eq!(*value, scalar(chunk * 256 + row as u64 + 1));
                                }
                                Ok(())
                            })
                            .unwrap();
                    }
                    snapshot
                        .with_column(layout, |values| {
                            assert_eq!(values.len(), 1 << k);
                            for (row, value) in values.iter().enumerate() {
                                assert_eq!(*value, scalar(row as u64 + 1));
                            }
                            Ok(())
                        })
                        .unwrap();
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 1);
                    assert!(!provider.window.get());
                }
            }
        }
        drop(sentinel);
        assert_eq!(provider.handles.live.get(), 0);
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn key_role_descriptor_substitution_is_retryable_but_authenticated_metadata_forgery_poisons() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart};
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for role in key_roles() {
            let mut replacements = key_roles()
                .into_iter()
                .filter(|other| *other != role)
                .map(|other| (Coefficient, other))
                .collect::<Vec<_>>();
            replacements.extend([
                (
                    Coefficient,
                    StoredPolynomialRoleV1::Advice {
                        column: 0,
                        phase: 0,
                    },
                ),
                (
                    Coefficient,
                    StoredPolynomialRoleV1::CopyPermutationProduct { set: 0 },
                ),
                (Coefficient, StoredPolynomialRoleV1::Instance { column: 0 }),
                (Coefficient, StoredPolynomialRoleV1::VanishingRandom),
                (
                    CosetPart {
                        extension_log: 1,
                        part: 0,
                    },
                    role,
                ),
                (
                    CosetPart {
                        extension_log: 1,
                        part: 1,
                    },
                    role,
                ),
            ]);
            for (other_basis, other_role) in replacements {
                for full_column in [false, true] {
                    let mut snapshot = key_snapshot(&mut provider, field, Coefficient, 1, role);
                    let original = snapshot.layout();
                    let wrong = StoredPolynomialLayoutV1::new(
                        provider.proof_context,
                        original.ordinal(),
                        field,
                        other_basis,
                        original.k(),
                        other_role,
                    )
                    .unwrap();
                    assert_ne!(wrong.context_digest(), original.context_digest());
                    let next = provider.next_ordinal;
                    // Refusal of a caller's false expected receipt does not touch the spool.
                    snapshot.injected_read_error = Some(StoredPolynomialErrorV1::Storage);
                    assert_eq!(
                        snapshot.with_chunk(wrong, 0, |_| panic!("wrong key role exposed chunk")),
                        Err::<(), _>(StoredPolynomialErrorV1::Context)
                    );
                    assert_eq!(
                        snapshot.with_column(wrong, |_| panic!("wrong key role exposed column")),
                        Err::<(), _>(StoredPolynomialErrorV1::Context)
                    );
                    assert_eq!(
                        snapshot.injected_read_error.take(),
                        Some(StoredPolynomialErrorV1::Storage)
                    );
                    snapshot
                        .with_chunk(original, 0, |values| {
                            assert_eq!(values, &[scalar(1), scalar(2)]);
                            Ok(())
                        })
                        .unwrap();
                    assert_eq!(provider.next_ordinal, next);
                    assert!(snapshot.raw.is_some());
                    // Private metadata corruption then reaches the actual spool's context gate.
                    snapshot.layout = wrong;
                    let result = if full_column {
                        snapshot.with_column(wrong, |_| panic!("forged key role exposed column"))
                    } else {
                        snapshot.with_chunk(wrong, 0, |_| panic!("forged key role exposed chunk"))
                    };
                    assert_eq!(result, Err::<(), _>(StoredPolynomialErrorV1::Context));
                    assert!(snapshot.raw.is_none());
                    assert_eq!(
                        snapshot.with_chunk(wrong, 0, |_| Ok(())),
                        Err(StoredPolynomialErrorV1::Poisoned)
                    );
                    assert_eq!(
                        snapshot.with_column(wrong, |_| Ok(())),
                        Err(StoredPolynomialErrorV1::Poisoned)
                    );
                    assert!(!provider.window.get());
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 0);
                }
            }
        }
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
