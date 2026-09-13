//! Product-role regressions using the actual encrypted Core provider and its shared budget.
//!
//! These tests do not construct a proof. Metadata forgery reaches the actual spool's
//! authenticated-context refusal; it is not an isolated AEAD ciphertext-corruption test.

use super::*;

fn product_roles(index: u32) -> [StoredPolynomialRoleV1; 2] {
    [
        StoredPolynomialRoleV1::CopyPermutationProduct { set: index },
        StoredPolynomialRoleV1::LookupProduct { lookup: index },
    ]
}

fn product_bases() -> [StoredPolynomialBasisV1; 4] {
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

fn product_value(row: usize) -> [u8; 32] {
    if row % 4 == 0 {
        [0; 32]
    } else {
        scalar(row as u64 * 17 + 1)
    }
}

fn filled_product(
    provider: &mut CoreStoredPolynomialProviderV1,
    field: StoredPastaFieldV1,
    basis: StoredPolynomialBasisV1,
    k: u32,
    role: StoredPolynomialRoleV1,
) -> CoreStoredPolynomialSnapshotV1 {
    let mut writer = provider.create(field, basis, k, role).unwrap();
    let layout = writer.layout();
    for chunk in 0..layout.chunk_count() as u64 {
        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
        let values = (0..layout.chunk_scalar_count(chunk).unwrap())
            .map(|offset| product_value(start + offset))
            .collect::<Vec<_>>();
        writer.write_chunk(chunk, &values).unwrap();
    }
    writer.seal().unwrap()
}

#[test]
fn product_roles_roundtrip_both_fields_bases_boundary_indexes_and_exact_chunk_geometry() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut previous = None;
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for index in [0, u32::MAX] {
            for role in product_roles(index) {
                for basis in product_bases() {
                    // A one-row and half-chunk tail, one complete chunk, then two chunks.
                    for k in [0, 7, 8, 9] {
                        let mut snapshot = filled_product(&mut provider, field, basis, k, role);
                        let layout = snapshot.layout();
                        assert_eq!(layout.role(), role);
                        assert_eq!(layout.field(), field);
                        assert_eq!(layout.basis(), basis);
                        assert_eq!(layout.k(), k);
                        assert_eq!(layout.scalar_count(), 1 << k);
                        assert_eq!(
                            layout.chunk_count(),
                            (1_usize << k).div_ceil(STORED_SCALARS_PER_CHUNK_V1)
                        );
                        if let Some(last) = previous {
                            assert!(layout.ordinal() > last);
                        }
                        previous = Some(layout.ordinal());
                        assert!(snapshot.raw.is_some());
                        assert_eq!(provider.handles.live.get(), 1);
                        for chunk in (0..layout.chunk_count() as u64).rev() {
                            snapshot
                                .with_chunk(layout, chunk, |values| {
                                    assert_eq!(
                                        values.len(),
                                        layout.chunk_scalar_count(chunk).unwrap()
                                    );
                                    for (offset, value) in values.iter().enumerate() {
                                        assert_eq!(
                                            *value,
                                            product_value(
                                                chunk as usize * STORED_SCALARS_PER_CHUNK_V1
                                                    + offset
                                            )
                                        );
                                    }
                                    Ok(())
                                })
                                .unwrap();
                        }
                        snapshot
                            .with_column(layout, |values| {
                                assert_eq!(values.len(), 1 << k);
                                for (row, value) in values.iter().enumerate() {
                                    assert_eq!(*value, product_value(row));
                                }
                                Ok(())
                            })
                            .unwrap();
                        assert!(!provider.window.get());
                        drop(snapshot);
                        assert_eq!(provider.handles.live.get(), 0);
                        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
                    }
                }
            }
        }
    }
}

#[test]
fn product_role_and_basis_forgeries_distinguish_retryable_mismatch_from_authenticated_poisoning() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut unrelated = filled(&mut provider, field, 1);
        for (role_index, role) in product_roles(3).into_iter().enumerate() {
            for basis in product_bases() {
                let mut replacements = vec![
                    (product_roles(3)[1 - role_index], basis),
                    (product_roles(4)[role_index], basis),
                    (
                        StoredPolynomialRoleV1::Advice {
                            column: 3,
                            phase: 0,
                        },
                        basis,
                    ),
                    (
                        StoredPolynomialRoleV1::LookupCompressed {
                            lookup: 3,
                            side: StoredLookupSideV1::Input,
                        },
                        basis,
                    ),
                    (
                        StoredPolynomialRoleV1::LookupPermuted {
                            lookup: 3,
                            side: StoredLookupSideV1::Table,
                        },
                        basis,
                    ),
                ];
                replacements.extend(
                    product_bases()
                        .into_iter()
                        .filter(|other| *other != basis)
                        .map(|other| (role, other)),
                );
                assert_eq!(replacements.len(), 8);
                for (other_role, other_basis) in replacements {
                    for full_column in [false, true] {
                        let mut snapshot = filled_product(&mut provider, field, basis, 1, role);
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
                        let next = provider.next_ordinal;
                        assert_eq!(
                            snapshot.with_chunk(substituted, 0, |_| panic!(
                                "wrong expected product layout exposed plaintext"
                            )),
                            Err::<(), _>(StoredPolynomialErrorV1::Context)
                        );
                        assert_eq!(
                            snapshot.with_column(substituted, |_| panic!(
                                "wrong expected product layout exposed column"
                            )),
                            Err::<(), _>(StoredPolynomialErrorV1::Context)
                        );
                        assert!(snapshot.raw.is_some());
                        assert_eq!(provider.next_ordinal, next);
                        assert_eq!(provider.handles.live.get(), 2);
                        snapshot
                            .with_column(original, |values| {
                                assert_eq!(values, &[product_value(0), product_value(1)]);
                                Ok(())
                            })
                            .unwrap();

                        // Only the Core metadata changes. The encrypted spool retains its
                        // original context and rejects the changed authenticated interpretation.
                        snapshot.layout = substituted;
                        let refused = if full_column {
                            snapshot.with_column(substituted, |_| {
                                panic!("forged product metadata exposed column")
                            })
                        } else {
                            snapshot.with_chunk(substituted, 0, |_| {
                                panic!("forged product metadata exposed chunk")
                            })
                        };
                        assert_eq!(refused, Err::<(), _>(StoredPolynomialErrorV1::Context));
                        assert!(snapshot.raw.is_none());
                        assert!(!provider.window.get());
                        assert_eq!(provider.next_ordinal, next);
                        assert_eq!(provider.handles.live.get(), 2);
                        assert_eq!(
                            snapshot.with_chunk(substituted, 0, |_| Ok(())),
                            Err(StoredPolynomialErrorV1::Poisoned)
                        );
                        assert_eq!(
                            snapshot.with_column(original, |_| Ok(())),
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
            // Actual successful decrypt followed by consumer refusal or unwind also consumes
            // only this product owner, while releasing the shared plaintext window.
            for unwind in [false, true] {
                let mut snapshot = filled_product(
                    &mut provider,
                    field,
                    StoredPolynomialBasisV1::Coefficient,
                    1,
                    role,
                );
                let layout = snapshot.layout();
                if unwind {
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| {
                            let _ = snapshot.with_column(
                                layout,
                                |values| -> Result<(), StoredPolynomialErrorV1> {
                                    assert_eq!(values, &[product_value(0), product_value(1)]);
                                    panic!("consumer unwind after real product decrypt");
                                },
                            );
                        }))
                        .is_err()
                    );
                } else {
                    assert_eq!(
                        snapshot.with_chunk(layout, 0, |values| {
                            assert_eq!(values, &[product_value(0), product_value(1)]);
                            Err::<(), _>(StoredPolynomialErrorV1::Consumer)
                        }),
                        Err(StoredPolynomialErrorV1::Consumer)
                    );
                }
                assert!(snapshot.raw.is_none());
                assert!(!provider.window.get());
                assert_eq!(
                    snapshot.with_column(layout, |_| Ok(())),
                    Err(StoredPolynomialErrorV1::Poisoned)
                );
                drop(snapshot);
                assert_eq!(provider.handles.live.get(), 1);
                unrelated
                    .with_chunk(unrelated.layout(), 0, |values| {
                        assert_eq!(values, &[scalar(1), scalar(2)]);
                        Ok(())
                    })
                    .unwrap();
            }
        }
        drop(unrelated);
        assert_eq!(provider.handles.live.get(), 0);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

#[test]
fn product_roles_share_existing_capacity_plaintext_window_and_nonrecycled_ordinals() {
    let directory = tempfile::tempdir().unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
        assert_eq!(provider.handles.limit, 512);
        provider.handles = Rc::new(LiveSnapshotBudget {
            live: Cell::new(0),
            limit: 4,
        });
        let [copy, lookup] = product_roles(0);
        let mut source = filled_product(
            &mut provider,
            field,
            StoredPolynomialBasisV1::Lagrange,
            1,
            copy,
        );
        let mut sibling = filled_product(
            &mut provider,
            field,
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 1,
                part: 1,
            },
            1,
            lookup,
        );
        let mut unrelated = filled(&mut provider, field, 1);
        let mut writer = provider
            .create(field, StoredPolynomialBasisV1::Coefficient, 1, copy)
            .unwrap();
        let destination = writer.layout();
        assert_eq!(provider.handles.live.get(), 4);
        let next = provider.next_ordinal;
        source
            .with_chunk(source.layout(), 0, |values| {
                assert_eq!(values, &[product_value(0), product_value(1)]);
                assert_eq!(
                    sibling.with_column(sibling.layout(), |_| panic!(
                        "overlapping product read exposed plaintext"
                    )),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                assert_eq!(
                    writer.write_chunk(0, &[product_value(0), product_value(1)]),
                    Err(StoredPolynomialErrorV1::Busy)
                );
                assert!(matches!(
                    provider.create(field, StoredPolynomialBasisV1::Coefficient, 1, lookup),
                    Err(StoredPolynomialErrorV1::Busy)
                ));
                assert_eq!(provider.next_ordinal, next);
                assert_eq!(provider.handles.live.get(), 4);
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            provider.create(field, StoredPolynomialBasisV1::Coefficient, 1, lookup),
            Err(StoredPolynomialErrorV1::Capacity)
        ));
        assert_eq!(provider.next_ordinal, next);
        assert_eq!(provider.handles.live.get(), 4);
        writer
            .write_chunk(0, &[product_value(0), product_value(1)])
            .unwrap();
        let mut coefficient = writer.seal().unwrap();
        assert_eq!(coefficient.layout(), destination);
        assert_eq!(provider.handles.live.get(), 4);
        for snapshot in [&mut sibling, &mut coefficient] {
            snapshot
                .with_column(snapshot.layout(), |values| {
                    assert_eq!(values, &[product_value(0), product_value(1)]);
                    Ok(())
                })
                .unwrap();
        }
        drop(source);
        assert_eq!(provider.handles.live.get(), 3);
        let fresh = provider
            .create(field, StoredPolynomialBasisV1::Coefficient, 1, lookup)
            .unwrap();
        assert_eq!(fresh.layout().ordinal(), next);
        assert!(fresh.layout().ordinal() > destination.ordinal());
        let fresh_ordinal = fresh.layout().ordinal();
        drop(fresh);
        assert_eq!(provider.handles.live.get(), 3);
        let replacement = provider
            .create(field, StoredPolynomialBasisV1::Coefficient, 1, copy)
            .unwrap();
        assert!(replacement.layout().ordinal() > fresh_ordinal);
        drop(replacement);
        // Exhausted ordinal admission must not consume a free handle or poison survivors.
        provider.next_ordinal = u64::MAX;
        assert!(matches!(
            provider.create(field, StoredPolynomialBasisV1::Coefficient, 1, lookup),
            Err(StoredPolynomialErrorV1::Capacity)
        ));
        assert_eq!(provider.next_ordinal, u64::MAX);
        assert_eq!(provider.handles.live.get(), 3);
        unrelated
            .with_column(unrelated.layout(), |values| {
                assert_eq!(values, &[scalar(1), scalar(2)]);
                Ok(())
            })
            .unwrap();
        drop((coefficient, sibling, unrelated));
        assert_eq!(provider.handles.live.get(), 0);
        assert!(!provider.window.get());
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
