//! Permuted role binding through the actual encrypted Core store, without full-proof credit.

use super::*;

fn bases() -> [StoredPolynomialBasisV1; 4] {
    [
        StoredPolynomialBasisV1::Lagrange,
        StoredPolynomialBasisV1::Coefficient,
        StoredPolynomialBasisV1::CosetPart {
            extension_log: 1,
            part: 0,
        },
        StoredPolynomialBasisV1::CosetPart {
            extension_log: 1,
            part: 1,
        },
    ]
}

fn filled_basis(
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
            .map(|offset| scalar(chunk * 256 + offset as u64 + 1))
            .collect::<Vec<_>>();
        writer.write_chunk(chunk, &values).unwrap();
    }
    writer.seal().unwrap()
}

#[test]
fn permuted_roles_roundtrip_all_bases_fields_sides_and_chunk_geometries() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut previous = None;
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
            for lookup in [0, u32::MAX] {
                let role = StoredPolynomialRoleV1::LookupPermuted { lookup, side };
                for basis in bases() {
                    for k in [1, 9] {
                        let mut snapshot = filled_basis(&mut provider, field, basis, k, role);
                        let layout = snapshot.layout();
                        assert_eq!(layout.role(), role);
                        assert_eq!(layout.field(), field);
                        assert_eq!(layout.basis(), basis);
                        assert_eq!(layout.k(), k);
                        if let Some(last) = previous {
                            assert!(layout.ordinal() > last);
                        }
                        previous = Some(layout.ordinal());
                        for chunk in (0..layout.chunk_count() as u64).rev() {
                            snapshot
                                .with_chunk(layout, chunk, |values| {
                                    assert_eq!(
                                        values.len(),
                                        layout.chunk_scalar_count(chunk).unwrap()
                                    );
                                    for (offset, value) in values.iter().enumerate() {
                                        assert_eq!(*value, scalar(chunk * 256 + offset as u64 + 1));
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
                        assert!(!provider.window.get());
                        drop(snapshot);
                        assert_eq!(provider.handles.live.get(), 0);
                    }
                }
            }
        }
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn permuted_role_and_basis_substitutions_reach_authenticated_backend_refusal() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut unrelated = filled(&mut provider, field, 1);
        for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
            let opposite = match side {
                StoredLookupSideV1::Input => StoredLookupSideV1::Table,
                StoredLookupSideV1::Table => StoredLookupSideV1::Input,
            };
            let role = StoredPolynomialRoleV1::LookupPermuted { lookup: 3, side };
            for basis in bases() {
                let mut replacements = vec![
                    (
                        StoredPolynomialRoleV1::LookupCompressed { lookup: 3, side },
                        basis,
                    ),
                    (
                        StoredPolynomialRoleV1::LookupPermuted { lookup: 4, side },
                        basis,
                    ),
                    (
                        StoredPolynomialRoleV1::LookupPermuted {
                            lookup: 3,
                            side: opposite,
                        },
                        basis,
                    ),
                    (
                        StoredPolynomialRoleV1::Advice {
                            column: 3,
                            phase: 0,
                        },
                        basis,
                    ),
                ];
                replacements.extend(
                    bases()
                        .into_iter()
                        .filter(|other| *other != basis)
                        .map(|other| (role, other)),
                );
                for (other_role, other_basis) in replacements {
                    for full_column in [false, true] {
                        let mut snapshot = filled_basis(&mut provider, field, basis, 1, role);
                        let original = snapshot.layout();
                        let substituted = StoredPolynomialLayoutV1::new(
                            provider.proof_context,
                            original.ordinal(),
                            field,
                            other_basis,
                            original.k(),
                            other_role,
                        )
                        .unwrap();
                        assert_ne!(original.context_digest(), substituted.context_digest());
                        assert_eq!(
                            snapshot.with_chunk(substituted, 0, |_| panic!(
                                "wrong expected layout exposed plaintext"
                            )),
                            Err::<(), _>(StoredPolynomialErrorV1::Context)
                        );
                        assert_eq!(
                            snapshot.with_column(substituted, |_| panic!(
                                "wrong expected layout exposed plaintext"
                            )),
                            Err::<(), _>(StoredPolynomialErrorV1::Context)
                        );
                        assert!(snapshot.raw.is_some());
                        snapshot
                            .with_chunk(original, 0, |values| {
                                assert_eq!(values, &[scalar(1), scalar(2)]);
                                Ok(())
                            })
                            .unwrap();

                        // Change only the Core-side expected metadata. The actual crypto spool
                        // retains its original authenticated context and must reject the forgery.
                        snapshot.layout = substituted;
                        let rejected = if full_column {
                            snapshot.with_column(substituted, |_| {
                                panic!("forged metadata exposed plaintext")
                            })
                        } else {
                            snapshot.with_chunk(substituted, 0, |_| {
                                panic!("forged metadata exposed plaintext")
                            })
                        };
                        assert_eq!(rejected, Err::<(), _>(StoredPolynomialErrorV1::Context));
                        assert!(snapshot.raw.is_none());
                        assert!(!provider.window.get());
                        assert_eq!(
                            snapshot.with_chunk(substituted, 0, |_| Ok(())),
                            Err(StoredPolynomialErrorV1::Poisoned)
                        );
                        drop(snapshot);
                        assert_eq!(provider.handles.live.get(), 1);
                        unrelated
                            .with_column(unrelated.layout(), |values| {
                                assert_eq!(values, &[scalar(1), scalar(2)]);
                                Ok(())
                            })
                            .unwrap();
                    }
                }
            }
        }
        drop(unrelated);
        assert_eq!(provider.handles.live.get(), 0);
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn permuted_bases_share_existing_window_capacity_and_monotonic_ordinals() {
    let directory = tempfile::tempdir().unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
        assert_eq!(provider.handles.limit, 512);
        provider.handles = Rc::new(LiveSnapshotBudget {
            live: Cell::new(0),
            limit: 4,
        });
        let input = StoredPolynomialRoleV1::LookupPermuted {
            lookup: 0,
            side: StoredLookupSideV1::Input,
        };
        let table = StoredPolynomialRoleV1::LookupPermuted {
            lookup: 0,
            side: StoredLookupSideV1::Table,
        };
        let mut source = filled_basis(
            &mut provider,
            field,
            StoredPolynomialBasisV1::Lagrange,
            1,
            input,
        );
        let mut sibling = filled_basis(
            &mut provider,
            field,
            StoredPolynomialBasisV1::Lagrange,
            1,
            table,
        );
        let unrelated = filled(&mut provider, field, 1);
        let mut writer = provider
            .create(field, StoredPolynomialBasisV1::Coefficient, 1, input)
            .unwrap();
        let destination = writer.layout();
        let next = provider.next_ordinal;
        source
            .with_chunk(source.layout(), 0, |_| {
                assert_eq!(
                    sibling.with_chunk(sibling.layout(), 0, |_| panic!(
                        "overlapping read exposed plaintext"
                    )),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                assert_eq!(
                    writer.write_chunk(0, &[scalar(1), scalar(2)]),
                    Err(StoredPolynomialErrorV1::Busy)
                );
                assert!(matches!(
                    provider.create(field, StoredPolynomialBasisV1::Coefficient, 1, table),
                    Err(StoredPolynomialErrorV1::Busy)
                ));
                assert_eq!(provider.next_ordinal, next);
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            provider.create(field, StoredPolynomialBasisV1::Coefficient, 1, table),
            Err(StoredPolynomialErrorV1::Capacity)
        ));
        assert_eq!(provider.next_ordinal, next);
        writer.write_chunk(0, &[scalar(1), scalar(2)]).unwrap();
        let coefficient = writer.seal().unwrap();
        sibling
            .with_chunk(sibling.layout(), 0, |values| {
                assert_eq!(values, &[scalar(1), scalar(2)]);
                Ok(())
            })
            .unwrap();
        drop(source);
        let fresh = provider
            .create(field, StoredPolynomialBasisV1::Coefficient, 1, table)
            .unwrap();
        assert!(fresh.layout().ordinal() > destination.ordinal());
        drop((fresh, coefficient, sibling, unrelated));
        assert_eq!(provider.handles.live.get(), 0);
        assert!(!provider.window.get());
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
