//! Actual-spool adapter tests; no mock cipher, digest authority, or proof qualification.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ff::WithSmallOrderMulGroup;
use halo2_proofs::{
    halo2curves::pasta::{Fp, Fq},
    plonk::Assigned,
    poly::{
        EvaluationDomain,
        stored_advice::{
            StoredLookupSideV1,
            assignment::{StoredAdviceAssignmentV1, StoredAssignmentFieldV1},
            transform::convert_stored_advice_v1,
        },
    },
};

use super::*;

fn scalar(value: u64) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn filled(
    provider: &mut CoreStoredPolynomialProviderV1,
    field: StoredPastaFieldV1,
    k: u32,
) -> CoreStoredPolynomialSnapshotV1 {
    filled_role(
        provider,
        field,
        k,
        StoredPolynomialRoleV1::Advice {
            column: 3,
            phase: 0,
        },
    )
}

fn filled_role(
    provider: &mut CoreStoredPolynomialProviderV1,
    field: StoredPastaFieldV1,
    k: u32,
    role: StoredPolynomialRoleV1,
) -> CoreStoredPolynomialSnapshotV1 {
    let mut writer = provider
        .create(field, StoredPolynomialBasisV1::Lagrange, k, role)
        .unwrap();
    let layout = writer.layout();
    for index in 0..layout.chunk_count() as u64 {
        let values = (0..layout.chunk_scalar_count(index).unwrap())
            .map(|row| scalar(index * 256 + row as u64 + 1))
            .collect::<Vec<_>>();
        writer.write_chunk(index, &values).unwrap();
    }
    writer.seal().unwrap()
}

fn distinct_roles() -> [StoredPolynomialRoleV1; 6] {
    use StoredLookupSideV1::{Input, Table};
    use StoredPolynomialRoleV1::{Advice, LookupCompressed};
    [
        Advice {
            column: 3,
            phase: 0,
        },
        Advice {
            column: 3,
            phase: 1,
        },
        Advice {
            column: 4,
            phase: 0,
        },
        LookupCompressed {
            lookup: 3,
            side: Input,
        },
        LookupCompressed {
            lookup: 3,
            side: Table,
        },
        LookupCompressed {
            lookup: 4,
            side: Input,
        },
    ]
}

#[test]
fn lookup_roles_roundtrip_both_pasta_fields_and_exact_chunk_tails() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for lookup in [0, u32::MAX] {
            for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
                let role = StoredPolynomialRoleV1::LookupCompressed { lookup, side };
                for k in [1, 9] {
                    let mut snapshot = filled_role(&mut provider, field, k, role);
                    let layout = snapshot.layout();
                    assert_eq!(layout.role(), role);
                    assert_eq!(layout.field(), field);
                    assert_eq!(layout.basis(), StoredPolynomialBasisV1::Lagrange);
                    for index in (0..layout.chunk_count() as u64).rev() {
                        snapshot
                            .with_chunk(layout, index, |values| {
                                assert_eq!(values.len(), layout.chunk_scalar_count(index).unwrap());
                                for (offset, value) in values.iter().enumerate() {
                                    assert_eq!(*value, scalar(index * 256 + offset as u64 + 1));
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
                }
            }
        }
    }
    assert_eq!(provider.handles.live.get(), 0);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn expected_role_kind_index_side_and_advice_phase_mismatches_are_retryable() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for role in distinct_roles() {
            let mut snapshot = filled_role(&mut provider, field, 1, role);
            let original = snapshot.layout();
            for substituted_role in distinct_roles() {
                if role == substituted_role {
                    continue;
                }
                let substituted = StoredPolynomialLayoutV1::new(
                    provider.proof_context,
                    original.ordinal(),
                    field,
                    original.basis(),
                    original.k(),
                    substituted_role,
                )
                .unwrap();
                assert_ne!(original.context_digest(), substituted.context_digest());
                assert_eq!(
                    snapshot.with_chunk(substituted, 0, |_| panic!("wrong role exposed a chunk")),
                    Err::<(), _>(StoredPolynomialErrorV1::Context)
                );
                assert_eq!(
                    snapshot.with_column(substituted, |_| panic!("wrong role exposed a column")),
                    Err::<(), _>(StoredPolynomialErrorV1::Context)
                );
                assert!(snapshot.raw.is_some());
                assert!(!provider.window.get());
            }
            snapshot
                .with_column(original, |values| {
                    assert_eq!(values, &[scalar(1), scalar(2)]);
                    Ok(())
                })
                .unwrap();
        }
    }
}

#[test]
fn trusted_role_substitution_reaches_backend_context_refusal_and_poisons_owner() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for role in distinct_roles() {
            for substituted_role in distinct_roles() {
                if role == substituted_role {
                    continue;
                }
                for full_column in [false, true] {
                    let mut snapshot = filled_role(&mut provider, field, 1, role);
                    let original = snapshot.layout();
                    let substituted = StoredPolynomialLayoutV1::new(
                        provider.proof_context,
                        original.ordinal(),
                        field,
                        original.basis(),
                        original.k(),
                        substituted_role,
                    )
                    .unwrap();
                    assert_ne!(original.context_digest(), substituted.context_digest());
                    // Bypass only Core's expected-layout equality using private test access.
                    // The crypto spool still owns the original context and rejects the new
                    // digest before decryption. This is backend context refusal, not an
                    // isolated AEAD test; raw layout/key/file mutation remains encapsulated.
                    snapshot.layout = substituted;
                    let result = if full_column {
                        snapshot
                            .with_column(substituted, |_| panic!("forged role exposed a column"))
                    } else {
                        snapshot
                            .with_chunk(substituted, 0, |_| panic!("forged role exposed a chunk"))
                    };
                    assert_eq!(result, Err::<(), _>(StoredPolynomialErrorV1::Context));
                    assert!(snapshot.raw.is_none());
                    assert!(!provider.window.get());
                    assert_eq!(
                        snapshot.with_column(substituted, |_| Ok(())),
                        Err(StoredPolynomialErrorV1::Poisoned)
                    );
                    drop(snapshot);
                    assert_eq!(provider.handles.live.get(), 0);
                    let mut sibling = filled_role(&mut provider, field, 1, role);
                    sibling
                        .with_chunk(sibling.layout(), 0, |values| {
                            assert_eq!(values, &[scalar(1), scalar(2)]);
                            Ok(())
                        })
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn advice_and_lookup_roles_share_one_plaintext_window_for_reads_and_writes() {
    let directory = tempfile::tempdir().unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
        let advice_role = StoredPolynomialRoleV1::Advice {
            column: 0,
            phase: 0,
        };
        let input_role = StoredPolynomialRoleV1::LookupCompressed {
            lookup: 0,
            side: StoredLookupSideV1::Input,
        };
        let table_role = StoredPolynomialRoleV1::LookupCompressed {
            lookup: 0,
            side: StoredLookupSideV1::Table,
        };
        let mut advice = filled_role(&mut provider, field, 1, advice_role);
        let mut input = filled_role(&mut provider, field, 1, input_role);
        let mut table = filled_role(&mut provider, field, 1, table_role);
        let mut pending = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 1, table_role)
            .unwrap();
        let mut complete = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 0, input_role)
            .unwrap();
        complete.write_chunk(0, &[scalar(1)]).unwrap();
        let ordinal = provider.next_ordinal;
        advice
            .with_column(advice.layout(), |_| {
                assert!(provider.window.get());
                assert_eq!(
                    input.with_chunk(input.layout(), 0, |_| panic!("nested input chunk")),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                assert_eq!(
                    table.with_column(table.layout(), |_| panic!("nested table column")),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                assert_eq!(
                    pending.write_chunk(0, &[scalar(1), scalar(2)]),
                    Err(StoredPolynomialErrorV1::Busy)
                );
                assert!(pending.raw.is_some());
                assert_eq!(pending.next_chunk, 0);
                assert!(matches!(
                    complete.seal(),
                    Err(StoredPolynomialErrorV1::Busy)
                ));
                assert!(matches!(
                    provider.create(field, StoredPolynomialBasisV1::Lagrange, 1, input_role),
                    Err(StoredPolynomialErrorV1::Busy)
                ));
                assert_eq!(provider.next_ordinal, ordinal);
                Ok(())
            })
            .unwrap();
        assert!(!provider.window.get());
        assert_eq!(
            provider.handles.live.get(),
            4,
            "failed consuming seal releases its lease"
        );
        input
            .with_chunk(input.layout(), 0, |_| {
                assert_eq!(
                    advice.with_column(advice.layout(), |_| panic!("nested advice column")),
                    Err::<(), _>(StoredPolynomialErrorV1::Busy)
                );
                Ok(())
            })
            .unwrap();
        pending.write_chunk(0, &[scalar(1), scalar(2)]).unwrap();
        let mut pending = pending.seal().unwrap();
        pending
            .with_column(pending.layout(), |values| {
                assert_eq!(values, &[scalar(1), scalar(2)]);
                Ok(())
            })
            .unwrap();
        table.with_column(table.layout(), |_| Ok(())).unwrap();
        advice.with_column(advice.layout(), |_| Ok(())).unwrap();
        drop((advice, input, table, pending));
        assert_eq!(provider.handles.live.get(), 0);
        assert!(!provider.window.get());
    }
}

#[test]
fn mixed_role_writers_and_snapshots_share_capacity_and_never_recycle_ordinals() {
    let directory = tempfile::tempdir().unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
        assert_eq!(provider.handles.limit, 512);
        provider.handles = Rc::new(LiveSnapshotBudget {
            live: Cell::new(0),
            limit: 3,
        });
        let advice_role = StoredPolynomialRoleV1::Advice {
            column: 0,
            phase: 0,
        };
        let input_role = StoredPolynomialRoleV1::LookupCompressed {
            lookup: 0,
            side: StoredLookupSideV1::Input,
        };
        let table_role = StoredPolynomialRoleV1::LookupCompressed {
            lookup: 0,
            side: StoredLookupSideV1::Table,
        };
        let advice = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 0, advice_role)
            .unwrap();
        let mut input = filled_role(&mut provider, field, 0, input_role);
        let table = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 0, table_role)
            .unwrap();
        assert_eq!(
            [
                advice.layout().ordinal(),
                input.layout().ordinal(),
                table.layout().ordinal()
            ],
            [0, 1, 2]
        );
        assert_eq!(provider.handles.live.get(), 3);
        for role in [advice_role, input_role, table_role] {
            assert!(matches!(
                provider.create(field, StoredPolynomialBasisV1::Lagrange, 0, role),
                Err(StoredPolynomialErrorV1::Capacity)
            ));
            assert_eq!(provider.next_ordinal, 3);
            assert_eq!(provider.handles.live.get(), 3);
        }
        drop(advice);
        assert_eq!(provider.handles.live.get(), 2);
        provider.directory = directory.path().join("missing-role-spool-directory");
        assert!(matches!(
            provider.create(field, StoredPolynomialBasisV1::Lagrange, 0, table_role),
            Err(StoredPolynomialErrorV1::Storage)
        ));
        assert_eq!(
            provider.next_ordinal, 4,
            "failed external creation burns the shared ordinal"
        );
        assert_eq!(provider.handles.live.get(), 2);
        assert!(!provider.window.get());
        provider.directory = directory.path().to_owned();
        let replacement = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 0, advice_role)
            .unwrap();
        assert_eq!(replacement.layout().ordinal(), 4);
        assert_eq!(provider.handles.live.get(), 3);
        input
            .with_column(input.layout(), |values| {
                assert_eq!(values, &[scalar(1)]);
                Ok(())
            })
            .unwrap();
        drop((input, table, replacement));
        assert_eq!(provider.handles.live.get(), 0);
        assert!(!provider.window.get());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

#[test]
fn both_pasta_fields_roundtrip_out_of_order_and_materialize_exactly_one_column() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut snapshot = filled(&mut provider, field, 9);
        let layout = snapshot.layout();
        for index in [1, 0, 1] {
            snapshot
                .with_chunk(layout, index, |values| {
                    assert_eq!(values.len(), 256);
                    for (offset, value) in values.iter().enumerate() {
                        assert_eq!(*value, scalar(index * 256 + offset as u64 + 1));
                    }
                    Ok(())
                })
                .unwrap();
        }
        snapshot
            .with_column(layout, |values| {
                assert_eq!(values.len(), 512);
                assert_eq!(
                    values
                        .iter()
                        .enumerate()
                        .filter(|(i, value)| **value != scalar(*i as u64 + 1))
                        .count(),
                    0
                );
                Ok(())
            })
            .unwrap();
        assert!(!provider.window.get());
    }
    assert_eq!(
        std::fs::read_dir(directory.path()).unwrap().count(),
        0,
        "spools are unlinked"
    );
}

#[test]
fn small_polynomial_padding_is_hidden_and_exact() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let layout = snapshot.layout();
    snapshot
        .with_chunk(layout, 0, |values| {
            assert_eq!(values, &[scalar(1), scalar(2)]);
            Ok(())
        })
        .unwrap();
    snapshot
        .with_column(layout, |values| {
            assert_eq!(values, &[scalar(1), scalar(2)]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn sequential_write_preflights_do_not_consume_valid_writer() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut writer = provider
        .create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            9,
            StoredPolynomialRoleV1::Advice {
                column: 1,
                phase: 0,
            },
        )
        .unwrap();
    let valid = vec![scalar(1); 256];
    assert_eq!(
        writer.write_chunk(1, &valid),
        Err(StoredPolynomialErrorV1::WriteOrder)
    );
    assert_eq!(
        writer.write_chunk(0, &valid[..255]),
        Err(StoredPolynomialErrorV1::WriteOrder)
    );
    let invalid = vec![[0xff; 32]; 256];
    assert_eq!(
        writer.write_chunk(0, &invalid),
        Err(StoredPolynomialErrorV1::Encoding)
    );
    writer.write_chunk(0, &valid).unwrap();
    assert_eq!(
        writer.write_chunk(0, &valid),
        Err(StoredPolynomialErrorV1::WriteOrder)
    );
    writer.write_chunk(1, &valid).unwrap();
    let mut snapshot = writer.seal().unwrap();
    let layout = snapshot.layout();
    snapshot
        .with_column(layout, |values| {
            assert!(values.iter().all(|value| *value == scalar(1)));
            Ok(())
        })
        .unwrap();
    let incomplete = provider
        .create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            StoredPolynomialRoleV1::Advice {
                column: 1,
                phase: 0,
            },
        )
        .unwrap();
    assert!(matches!(
        incomplete.seal(),
        Err(StoredPolynomialErrorV1::Incomplete)
    ));
}

#[test]
fn expected_metadata_and_slot_mismatches_are_retryable_and_never_expose_plaintext() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let original = snapshot.layout();
    let mut wrong = Vec::new();
    for (field, basis, k, column, phase) in [
        (
            StoredPastaFieldV1::Fq,
            StoredPolynomialBasisV1::Lagrange,
            1,
            3,
            0,
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            1,
            3,
            0,
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            2,
            3,
            0,
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            1,
            4,
            0,
        ),
        (
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            1,
            3,
            1,
        ),
    ] {
        wrong.push(
            StoredPolynomialLayoutV1::new(
                provider.proof_context,
                original.ordinal(),
                field,
                basis,
                k,
                StoredPolynomialRoleV1::Advice { column, phase },
            )
            .unwrap(),
        );
    }
    wrong.push(
        StoredPolynomialLayoutV1::new(
            provider.proof_context,
            original.ordinal() + 1,
            original.field(),
            original.basis(),
            original.k(),
            original.role(),
        )
        .unwrap(),
    );
    wrong.push(
        StoredPolynomialLayoutV1::new(
            [9; 32],
            original.ordinal(),
            original.field(),
            original.basis(),
            original.k(),
            original.role(),
        )
        .unwrap(),
    );
    for expected in wrong {
        assert_eq!(
            snapshot.with_column(expected, |_| panic!("wrong metadata exposed plaintext")),
            Err::<(), _>(StoredPolynomialErrorV1::Context)
        );
        assert_eq!(
            snapshot.with_chunk(expected, 0, |_| panic!("wrong metadata exposed plaintext")),
            Err::<(), _>(StoredPolynomialErrorV1::Context)
        );
    }
    assert_eq!(
        snapshot.with_chunk(original, 1, |_| panic!("invalid slot exposed plaintext")),
        Err::<(), _>(StoredPolynomialErrorV1::ChunkIndex)
    );
    snapshot.with_column(original, |_| Ok(())).unwrap();
}

#[test]
fn shared_window_blocks_nested_materializations_and_recovers_after_release() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut first = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let mut second = filled(&mut provider, StoredPastaFieldV1::Fq, 1);
    let second_layout = second.layout();
    first
        .with_column(first.layout(), |_| {
            assert_eq!(
                second.with_column(second_layout, |_| panic!("two columns materialized")),
                Err::<(), _>(StoredPolynomialErrorV1::Busy)
            );
            assert_eq!(
                second.with_chunk(second_layout, 0, |_| panic!("nested chunk materialized")),
                Err::<(), _>(StoredPolynomialErrorV1::Busy)
            );
            assert!(matches!(
                provider.create(
                    StoredPastaFieldV1::Fp,
                    StoredPolynomialBasisV1::Lagrange,
                    1,
                    StoredPolynomialRoleV1::Advice {
                        column: 0,
                        phase: 0
                    }
                ),
                Err(StoredPolynomialErrorV1::Busy)
            ));
            Ok(())
        })
        .unwrap();
    second.with_column(second_layout, |_| Ok(())).unwrap();
    assert!(!provider.window.get());
}

#[test]
fn consumer_errors_and_panics_poison_only_the_affected_snapshot_and_release_window() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for full_column in [false, true] {
        for panic in [false, true] {
            let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
            let layout = snapshot.layout();
            let result = catch_unwind(AssertUnwindSafe(|| {
                let consume = |_: &[[u8; 32]]| -> Result<(), StoredPolynomialErrorV1> {
                    if panic {
                        panic!("consumer panic");
                    }
                    Err(StoredPolynomialErrorV1::Consumer)
                };
                if full_column {
                    snapshot.with_column(layout, consume)
                } else {
                    snapshot.with_chunk(layout, 0, consume)
                }
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(StoredPolynomialErrorV1::Consumer));
            }
            assert!(snapshot.raw.is_none());
            assert!(!provider.window.get());
            assert_eq!(
                snapshot.with_chunk(layout, 0, |_| Ok(())),
                Err(StoredPolynomialErrorV1::Poisoned)
            );
            let mut sibling = filled(&mut provider, StoredPastaFieldV1::Fq, 1);
            sibling.with_column(sibling.layout(), |_| Ok(())).unwrap();
        }
    }
}

#[test]
fn injected_operational_failures_and_unwind_leave_no_live_read_owner() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for error in [
        StoredPolynomialErrorV1::Allocation,
        StoredPolynomialErrorV1::Storage,
        StoredPolynomialErrorV1::Authentication,
    ] {
        let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
        let layout = snapshot.layout();
        snapshot.injected_read_error = Some(error);
        assert_eq!(
            snapshot.with_column(layout, |_| panic!("failed read exposed plaintext")),
            Err::<(), _>(error)
        );
        assert!(snapshot.raw.is_none());
        assert!(!provider.window.get());
        assert_eq!(
            snapshot.with_column(layout, |_| Ok(())),
            Err(StoredPolynomialErrorV1::Poisoned)
        );
    }
    let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let layout = snapshot.layout();
    snapshot.panic_on_read = true;
    assert!(
        catch_unwind(AssertUnwindSafe(
            || snapshot.with_chunk(layout, 0, |_| Ok(()))
        ))
        .is_err()
    );
    assert!(snapshot.raw.is_none());
    assert!(!provider.window.get());
}

#[test]
fn authenticated_but_noncanonical_scalars_or_tail_padding_fail_before_callback() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for malformed_padding in [false, true] {
        // Test-only construction bypasses the adapter writer to supply an authenticated
        // malformed record. No raw-backend constructor is exposed by the production adapter.
        let writer = provider
            .create(
                StoredPastaFieldV1::Fp,
                StoredPolynomialBasisV1::Lagrange,
                1,
                StoredPolynomialRoleV1::Advice {
                    column: 0,
                    phase: 0,
                },
            )
            .unwrap();
        let layout = writer.layout;
        let mut plaintext = ConfidentialSpoolChunkV1::new_zeroed_v1(CHUNK_BYTES as u64).unwrap();
        if malformed_padding {
            plaintext.as_mut_slice_v1()[64] = 1;
        } else {
            plaintext.as_mut_slice_v1()[..32].fill(0xff);
        }
        let mut raw = writer.raw.unwrap();
        raw.write_slot_v1(0, plaintext).unwrap();
        let mut snapshot = CoreStoredPolynomialSnapshotV1 {
            layout,
            raw: Some(raw.seal_v1().unwrap()),
            _lease: writer.lease,
            window: Rc::clone(&provider.window),
            injected_read_error: None,
            panic_on_read: false,
        };
        assert_eq!(
            snapshot.with_column(layout, |_| panic!("malformed bytes exposed")),
            Err::<(), _>(StoredPolynomialErrorV1::Encoding)
        );
        assert!(snapshot.raw.is_none());
        assert!(!provider.window.get());
    }
}

#[test]
fn provider_contexts_are_fresh_ordinals_monotonic_and_handle_count_bounded() {
    let directory = tempfile::tempdir().unwrap();
    let mut first = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let second = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    assert_ne!(first.proof_context, second.proof_context);
    let a = filled(&mut first, StoredPastaFieldV1::Fp, 0);
    let b = filled(&mut first, StoredPastaFieldV1::Fp, 0);
    assert_eq!(b.layout().ordinal(), a.layout().ordinal() + 1);
    assert_ne!(a.layout().context_digest(), b.layout().context_digest());
    first.next_ordinal = u64::MAX;
    assert!(matches!(
        first.create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            0,
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0
            }
        ),
        Err(StoredPolynomialErrorV1::Capacity)
    ));
}

#[test]
fn released_snapshots_allow_more_than_the_live_limit_without_reusing_identity() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut identities = std::collections::BTreeSet::new();
    for ordinal in 0..(MAX_LIVE_SNAPSHOTS_PER_PROOF + 1) as u64 {
        let snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 0);
        assert_eq!(snapshot.layout().ordinal(), ordinal);
        assert!(identities.insert(snapshot.layout().context_digest()));
        assert_eq!(provider.handles.live.get(), 1);
        drop(snapshot);
        assert_eq!(provider.handles.live.get(), 0);
    }
}

#[test]
fn live_writer_and_snapshot_share_one_quota_and_failed_creation_releases_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    // Exercise the production admission logic with a small quota, without requiring
    // more descriptors than the host's process limit.
    provider.handles = Rc::new(LiveSnapshotBudget {
        live: Cell::new(0),
        limit: 2,
    });
    let create = |provider: &mut CoreStoredPolynomialProviderV1| {
        provider.create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            0,
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0,
            },
        )
    };
    let mut writer = create(&mut provider).unwrap();
    let other = create(&mut provider).unwrap();
    let ordinal = provider.next_ordinal;
    assert!(matches!(
        create(&mut provider),
        Err(StoredPolynomialErrorV1::Capacity)
    ));
    assert_eq!(provider.next_ordinal, ordinal);
    writer.write_chunk(0, &[scalar(3)]).unwrap();
    let snapshot = writer.seal().unwrap();
    assert_eq!(provider.handles.live.get(), 2, "seal moves the lease");
    assert!(matches!(
        create(&mut provider),
        Err(StoredPolynomialErrorV1::Capacity)
    ));
    drop(other);
    assert_eq!(provider.handles.live.get(), 1);
    let incomplete = create(&mut provider).unwrap();
    assert!(matches!(
        incomplete.seal(),
        Err(StoredPolynomialErrorV1::Incomplete)
    ));
    assert_eq!(provider.handles.live.get(), 1);
    provider.directory = directory.path().join("missing-directory");
    let failed_ordinal = provider.next_ordinal;
    assert!(create(&mut provider).is_err());
    assert_eq!(provider.next_ordinal, failed_ordinal + 1);
    assert_eq!(provider.handles.live.get(), 1, "failed I/O releases quota");
    provider.directory = directory.path().to_owned();
    let replacement = create(&mut provider).unwrap();
    assert_eq!(replacement.layout().ordinal(), failed_ordinal + 1);
    drop(snapshot);
    drop(replacement);
    assert_eq!(provider.handles.live.get(), 0);
}

#[test]
fn raw_snapshot_substitution_and_late_decoding_failure_poison_before_exposure() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let original = snapshot.layout();
    // A test-only metadata substitution bypasses the public expected-layout preflight.
    // The crypto spool must still reject its distinct authenticated context.
    snapshot.layout = StoredPolynomialLayoutV1::new(
        provider.proof_context,
        original.ordinal(),
        StoredPastaFieldV1::Fq,
        original.basis(),
        original.k(),
        original.role(),
    )
    .unwrap();
    assert_eq!(
        snapshot.with_column(snapshot.layout(), |_| panic!("substituted field exposed")),
        Err::<(), _>(StoredPolynomialErrorV1::Context)
    );
    assert!(snapshot.raw.is_none());

    let writer = provider
        .create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0,
            },
        )
        .unwrap();
    let layout = writer.layout();
    let mut raw = writer.raw.unwrap();
    raw.write_slot_v1(
        0,
        ConfidentialSpoolChunkV1::new_zeroed_v1(CHUNK_BYTES as u64).unwrap(),
    )
    .unwrap();
    let mut malformed = ConfidentialSpoolChunkV1::new_zeroed_v1(CHUNK_BYTES as u64).unwrap();
    malformed.as_mut_slice_v1()[..32].fill(0xff);
    raw.write_slot_v1(1, malformed).unwrap();
    let mut snapshot = CoreStoredPolynomialSnapshotV1 {
        layout,
        raw: Some(raw.seal_v1().unwrap()),
        _lease: writer.lease,
        window: Rc::clone(&provider.window),
        injected_read_error: None,
        panic_on_read: false,
    };
    assert_eq!(
        snapshot.with_column(layout, |_| panic!("partially decoded column exposed")),
        Err::<(), _>(StoredPolynomialErrorV1::Encoding)
    );
    assert!(snapshot.raw.is_none());
    assert!(!provider.window.get());
}

#[test]
fn spool_errors_have_coarse_nonsecret_failure_classes() {
    for (raw, expected) in [
        (
            ConfidentialSpoolErrorV1::Authentication,
            StoredPolynomialErrorV1::Authentication,
        ),
        (
            ConfidentialSpoolErrorV1::Allocation("test allocation"),
            StoredPolynomialErrorV1::Allocation,
        ),
        (
            ConfidentialSpoolErrorV1::FileOperation {
                operation: "test read",
                kind: std::io::ErrorKind::UnexpectedEof,
            },
            StoredPolynomialErrorV1::Storage,
        ),
        (
            ConfidentialSpoolErrorV1::ContextDigestMismatch,
            StoredPolynomialErrorV1::Context,
        ),
        (
            ConfidentialSpoolErrorV1::Poisoned,
            StoredPolynomialErrorV1::Poisoned,
        ),
        (
            ConfidentialSpoolErrorV1::EntropyUnavailable,
            StoredPolynomialErrorV1::Backend,
        ),
    ] {
        let error = map_spool_error(raw);
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("test "));
    }
}

fn encrypted_assignment_roundtrip<F: StoredAssignmentFieldV1>() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let usable = 1019;
    let mut columns = Vec::new();
    for column in [7, 11] {
        let writer = provider
            .create(
                F::STORED_FIELD,
                StoredPolynomialBasisV1::Lagrange,
                10,
                StoredPolynomialRoleV1::Advice { column, phase: 2 },
            )
            .unwrap();
        let layout = writer.layout();
        columns.push(StoredAdviceAssignmentV1::<F, _>::new(writer, layout, usable).unwrap());
    }
    assert_eq!(provider.handles.live.get(), 2);
    // Full vectors below are test-only oracles, never the assignment/store implementation.
    let mut expected = [vec![F::ZERO.to_repr(); 1024], vec![F::ZERO.to_repr(); 1024]];
    for (column, row, value) in [
        (0, 0, Assigned::Trivial(F::from(19))),
        (1, 3, Assigned::Rational(F::from(25), F::from(5))),
        (0, 255, Assigned::Rational(F::from(33), F::from(3))),
        (1, 255, Assigned::Rational(F::from(91), F::ZERO)),
        (0, 256, Assigned::Zero),
        (0, 257, Assigned::Rational(F::ZERO, F::from(7))),
        (1, 768, Assigned::Rational(F::from(65), F::from(13))),
        (0, 1001, Assigned::Trivial(F::from(43))),
        (1, 1018, Assigned::Rational(F::from(77), F::from(11))),
    ] {
        columns[column].assign_discarding_value(row, value).unwrap();
        expected[column][row] = value.evaluate().to_repr();
    }
    let mut tail_calls = Vec::new();
    let mut snapshots = Vec::new();
    for (column, assignment) in columns.into_iter().enumerate() {
        snapshots.push(
            assignment
                .finish_with_tail(|row| {
                    tail_calls.push((column, row));
                    let value = F::from((column * 1024 + row + 100) as u64);
                    expected[column][row] = value.to_repr();
                    Ok(value)
                })
                .unwrap(),
        );
    }
    assert_eq!(
        tail_calls,
        (0..2)
            .flat_map(|column| (usable..1024).map(move |row| (column, row)))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        provider.handles.live.get(),
        2,
        "seal transfers each live lease"
    );
    let other_layout = snapshots[1].layout();
    assert_eq!(
        snapshots[0].with_column(other_layout, |_| panic!("another column exposed plaintext")),
        Err::<(), _>(StoredPolynomialErrorV1::Context)
    );
    for (column, snapshot) in snapshots.iter_mut().enumerate() {
        let layout = snapshot.layout();
        assert_eq!(layout.field(), F::STORED_FIELD);
        let StoredPolynomialRoleV1::Advice {
            column: actual_column,
            phase,
        } = layout.role()
        else {
            panic!("assignment emitted a non-advice role");
        };
        assert_eq!(phase, 2);
        assert_eq!(actual_column, [7, 11][column]);
        assert!(
            snapshot.raw.is_some(),
            "reads use the real authenticated spool"
        );
        for chunk in [3, 0, 2, 1, 3] {
            snapshot
                .with_chunk(layout, chunk, |values| {
                    let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
                    assert_eq!(values, &expected[column][start..start + 256]);
                    Ok(())
                })
                .unwrap();
        }
        snapshot
            .with_column(layout, |values| {
                assert_eq!(values, expected[column]);
                Ok(())
            })
            .unwrap();
        assert!(!provider.window.get());
    }
    drop(snapshots);
    assert_eq!(provider.handles.live.get(), 0);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn discard_assignment_uses_encrypted_store_for_both_fields_with_interleaved_gaps_and_tails() {
    encrypted_assignment_roundtrip::<Fp>();
    encrypted_assignment_roundtrip::<Fq>();
}

fn encrypted_assignment_releases_predecessors<F: StoredAssignmentFieldV1>() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    provider.handles = Rc::new(LiveSnapshotBudget {
        live: Cell::new(0),
        limit: 2,
    });
    let mut predecessor: Option<(CoreStoredPolynomialSnapshotV1, [[u8; 32]; 4])> = None;
    for generation in 0..6_u64 {
        let writer = provider
            .create(
                F::STORED_FIELD,
                StoredPolynomialBasisV1::Lagrange,
                2,
                StoredPolynomialRoleV1::Advice {
                    column: 9,
                    phase: 1,
                },
            )
            .unwrap();
        let layout = writer.layout();
        assert_eq!(
            layout.ordinal(),
            generation,
            "rejected admissions do not reuse or burn an ordinal"
        );
        let mut assignment = StoredAdviceAssignmentV1::<F, _>::new(writer, layout, 3).unwrap();
        let value = F::from(generation + 1);
        assignment
            .assign_discarding_value(1, Assigned::Rational(value * F::from(3), F::from(3)))
            .unwrap();
        let mut snapshot = assignment
            .finish_with_tail(|row| {
                assert_eq!(row, 3);
                Ok(F::from(generation + 2))
            })
            .unwrap();
        let expected = [
            F::ZERO.to_repr(),
            value.to_repr(),
            F::ZERO.to_repr(),
            F::from(generation + 2).to_repr(),
        ];
        snapshot
            .with_chunk(layout, 0, |values| {
                assert_eq!(values, expected);
                Ok(())
            })
            .unwrap();
        if let Some((mut old, old_expected)) = predecessor.take() {
            assert_eq!(provider.handles.live.get(), 2);
            assert!(matches!(
                provider.create(
                    F::STORED_FIELD,
                    StoredPolynomialBasisV1::Lagrange,
                    2,
                    StoredPolynomialRoleV1::Advice {
                        column: 9,
                        phase: 1
                    }
                ),
                Err(StoredPolynomialErrorV1::Capacity)
            ));
            let old_layout = old.layout();
            assert_ne!(layout.context_digest(), old_layout.context_digest());
            assert_eq!(
                snapshot.with_chunk(old_layout, 0, |_| panic!(
                    "predecessor identity exposed replacement"
                )),
                Err::<(), _>(StoredPolynomialErrorV1::Context)
            );
            old.with_column(old_layout, |values| {
                assert_eq!(values, old_expected);
                Ok(())
            })
            .unwrap();
            drop(old);
        }
        assert_eq!(provider.handles.live.get(), 1);
        assert!(!provider.window.get());
        predecessor = Some((snapshot, expected));
    }
    drop(predecessor);
    assert_eq!(provider.handles.live.get(), 0);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn assignment_snapshot_replacement_releases_encrypted_owners_without_reusing_identity() {
    encrypted_assignment_releases_predecessors::<Fp>();
    encrypted_assignment_releases_predecessors::<Fq>();
}

fn encrypted_basis_conversion_roundtrip<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>>() {
    use StoredPolynomialBasisV1::{Coefficient, CosetPart, Lagrange};

    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    // Only source and destination may coexist. Each conversion must release its predecessor
    // before a third authenticated owner can be created, with no lifetime creation quota.
    provider.handles = Rc::new(LiveSnapshotBudget {
        live: Cell::new(0),
        limit: 2,
    });
    let domain = EvaluationDomain::<F>::new(5, 9);
    let writer = provider
        .create(
            F::STORED_FIELD,
            Lagrange,
            9,
            StoredPolynomialRoleV1::Advice {
                column: 17,
                phase: 1,
            },
        )
        .unwrap();
    let initial_layout = writer.layout();
    let mut assignment =
        StoredAdviceAssignmentV1::<F, _>::new(writer, initial_layout, 507).unwrap();
    // Test-only full-column oracle: ordinary Assigned evaluation and existing public domain
    // arithmetic provide the reference, independently of the new stored transform helper.
    let mut evaluations = vec![F::ZERO; 512];
    for (row, value) in [
        (0, Assigned::Trivial(F::from(17))),
        (77, Assigned::Rational(F::from(21), F::from(7))),
        (255, Assigned::Rational(F::from(93), F::ZERO)),
        (256, Assigned::Trivial(F::from(35))),
        (506, Assigned::Rational(F::from(55), F::from(11))),
    ] {
        assignment.assign_discarding_value(row, value).unwrap();
        evaluations[row] = value.evaluate();
    }
    let mut current = assignment
        .finish_with_tail(|row| {
            let value = F::from((row * 7 + 1) as u64);
            evaluations[row] = value;
            Ok(value)
        })
        .unwrap();
    let coefficients = domain.lagrange_to_coeff(domain.lagrange_from_vec(evaluations.clone()));
    let extension_log = domain.extended_k() - domain.k();
    assert_eq!(extension_log, 2);
    let original_bytes = evaluations
        .iter()
        .map(|value| value.to_repr())
        .collect::<Vec<_>>();
    let mut predecessor_bytes = original_bytes.clone();
    current
        .with_column(initial_layout, |values| {
            assert_eq!(values, original_bytes);
            Ok(())
        })
        .unwrap();

    for basis in [
        Coefficient,
        CosetPart {
            extension_log,
            part: 0,
        },
        CosetPart {
            extension_log,
            part: 1,
        },
        Lagrange,
    ] {
        let expected_bytes = match basis {
            Coefficient => coefficients
                .iter()
                .map(|value| value.to_repr())
                .collect::<Vec<_>>(),
            CosetPart { part, .. } => domain
                .coeff_to_extended_part(
                    coefficients.clone(),
                    domain.get_extended_omega().pow_vartime([u64::from(part)]),
                )
                .iter()
                .map(|value| value.to_repr())
                .collect(),
            Lagrange => original_bytes.clone(),
        };
        let previous_layout = current.layout();
        assert!(!provider.window.get());
        let mut converted =
            convert_stored_advice_v1(&domain, &mut provider, &mut current, previous_layout, basis)
                .unwrap();
        assert!(
            !provider.window.get(),
            "every read callback ends before a write begins"
        );
        assert_eq!(provider.handles.live.get(), 2);
        let converted_layout = converted.layout();
        assert_eq!(converted_layout.ordinal(), previous_layout.ordinal() + 1);
        assert_eq!(converted_layout.field(), initial_layout.field());
        assert_eq!(converted_layout.k(), initial_layout.k());
        let (
            StoredPolynomialRoleV1::Advice {
                column: converted_column,
                phase: converted_phase,
            },
            StoredPolynomialRoleV1::Advice {
                column: initial_column,
                phase: initial_phase,
            },
        ) = (converted_layout.role(), initial_layout.role())
        else {
            panic!("advice conversion changed its polynomial role");
        };
        assert_eq!(converted_column, initial_column);
        assert_eq!(converted_phase, initial_phase);
        assert_eq!(converted_layout.basis(), basis);
        assert_ne!(
            converted_layout.context_digest(),
            previous_layout.context_digest()
        );
        assert!(current.raw.is_some() && converted.raw.is_some());
        assert_eq!(
            converted.with_column(previous_layout, |_| panic!(
                "old basis identity exposed output"
            )),
            Err::<(), _>(StoredPolynomialErrorV1::Context)
        );
        // Read the source separately, after conversion. It remains authenticated and unchanged.
        current
            .with_column(previous_layout, |values| {
                assert_eq!(values, predecessor_bytes);
                Ok(())
            })
            .unwrap();
        for chunk in [1, 0, 1] {
            converted
                .with_chunk(converted_layout, chunk, |values| {
                    let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
                    assert_eq!(values, &expected_bytes[start..start + 256]);
                    Ok(())
                })
                .unwrap();
        }
        drop(current);
        assert_eq!(provider.handles.live.get(), 1);
        predecessor_bytes = expected_bytes;
        current = converted;
    }
    assert_eq!(current.layout().basis(), Lagrange);
    assert_eq!(current.layout().ordinal(), initial_layout.ordinal() + 4);
    current
        .with_column(current.layout(), |values| {
            assert_eq!(values, original_bytes);
            Ok(())
        })
        .unwrap();
    drop(current);
    assert_eq!(provider.handles.live.get(), 0);
    assert!(!provider.window.get());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn encrypted_assignment_basis_conversions_match_existing_arithmetic_in_both_pasta_fields() {
    encrypted_basis_conversion_roundtrip::<Fp>();
    encrypted_basis_conversion_roundtrip::<Fq>();
}

#[test]
fn sorted_scratch_roles_roundtrip_actual_spools_with_short_and_merge_passes() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for (k, run_log) in [(1, 1), (9, 8), (9, 9)] {
            for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
                let role = StoredPolynomialRoleV1::LookupSorted {
                    lookup: u32::MAX,
                    side,
                    run_log,
                };
                let mut snapshot = filled_role(&mut provider, field, k, role);
                let layout = snapshot.layout();
                assert_eq!(layout.role(), role);
                for chunk in (0..layout.chunk_count() as u64).rev() {
                    snapshot
                        .with_chunk(layout, chunk, |values| {
                            assert_eq!(values.len(), layout.chunk_scalar_count(chunk).unwrap());
                            for (offset, value) in values.iter().enumerate() {
                                assert_eq!(*value, scalar(chunk * 256 + offset as u64 + 1));
                            }
                            Ok(())
                        })
                        .unwrap();
                }
            }
        }
    }
    assert_eq!(provider.handles.live.get(), 0);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn sorted_pass_substitution_is_refused_by_expected_layout_and_actual_spool_context() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
    let source_role = StoredPolynomialRoleV1::LookupSorted {
        lookup: 0,
        side: StoredLookupSideV1::Input,
        run_log: 8,
    };
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for replacement in [
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 0,
                side: StoredLookupSideV1::Input,
                run_log: 9,
            },
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 0,
                side: StoredLookupSideV1::Table,
                run_log: 8,
            },
            StoredPolynomialRoleV1::LookupSorted {
                lookup: 1,
                side: StoredLookupSideV1::Input,
                run_log: 8,
            },
            StoredPolynomialRoleV1::LookupCompressed {
                lookup: 0,
                side: StoredLookupSideV1::Input,
            },
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0,
            },
        ] {
            for full_column in [false, true] {
                let mut snapshot = filled_role(&mut provider, field, 9, source_role);
                let original = snapshot.layout();
                let replaced = StoredPolynomialLayoutV1::new(
                    provider.proof_context,
                    original.ordinal(),
                    field,
                    original.basis(),
                    original.k(),
                    replacement,
                )
                .unwrap();
                assert_ne!(original.context_digest(), replaced.context_digest());
                assert_eq!(
                    snapshot.with_chunk(replaced, 0, |_| panic!(
                        "substituted pass exposed plaintext"
                    )),
                    Err::<(), _>(StoredPolynomialErrorV1::Context)
                );
                assert!(snapshot.raw.is_some());
                snapshot.with_chunk(original, 0, |_| Ok(())).unwrap();
                // Test-only bypass of the adapter's equality check: the underlying encrypted
                // spool must still reject its original AAD under the substituted pass identity.
                snapshot.layout = replaced;
                let refused = if full_column {
                    snapshot.with_column(replaced, |_| panic!("substituted pass exposed column"))
                } else {
                    snapshot.with_chunk(replaced, 0, |_| panic!("substituted pass exposed chunk"))
                };
                assert_eq!(refused, Err::<(), _>(StoredPolynomialErrorV1::Context));
                assert!(snapshot.raw.is_none());
                assert_eq!(
                    snapshot.with_chunk(replaced, 0, |_| Ok(())),
                    Err(StoredPolynomialErrorV1::Poisoned)
                );
                drop(snapshot);
                assert_eq!(provider.handles.live.get(), 0);
                assert!(!provider.window.get());
            }
        }
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn sorted_scratch_uses_existing_shared_window_capacity_and_monotonic_ordinals() {
    let directory = tempfile::tempdir().unwrap();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        let mut provider = CoreStoredPolynomialProviderV1::new(directory.path()).unwrap();
        assert_eq!(provider.handles.limit, 512);
        provider.handles = Rc::new(LiveSnapshotBudget {
            live: Cell::new(0),
            limit: 2,
        });
        let role = StoredPolynomialRoleV1::LookupSorted {
            lookup: 0,
            side: StoredLookupSideV1::Input,
            run_log: 1,
        };
        let mut source = filled_role(&mut provider, field, 1, role);
        let mut writer = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 1, role)
            .unwrap();
        let destination = writer.layout();
        assert!(source.layout().ordinal() < destination.ordinal());
        let next = provider.next_ordinal;
        source
            .with_chunk(source.layout(), 0, |_| {
                assert_eq!(
                    writer.write_chunk(0, &[scalar(1), scalar(2)]),
                    Err(StoredPolynomialErrorV1::Busy)
                );
                assert!(matches!(
                    provider.create(field, StoredPolynomialBasisV1::Lagrange, 1, role),
                    Err(StoredPolynomialErrorV1::Busy)
                ));
                assert_eq!(provider.next_ordinal, next);
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            provider.create(field, StoredPolynomialBasisV1::Lagrange, 1, role),
            Err(StoredPolynomialErrorV1::Capacity)
        ));
        assert_eq!(provider.next_ordinal, next);
        writer.write_chunk(0, &[scalar(1), scalar(2)]).unwrap();
        let replacement = writer.seal().unwrap();
        drop(source);
        let fresh = provider
            .create(field, StoredPolynomialBasisV1::Lagrange, 1, role)
            .unwrap();
        assert!(fresh.layout().ordinal() > destination.ordinal());
        drop((fresh, replacement));
        assert_eq!(provider.handles.live.get(), 0);
        assert!(!provider.window.get());
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[path = "permuted_roles.rs"]
mod permuted_roles;

#[path = "products_roles.rs"]
mod products_roles;

#[path = "vanishing_roles.rs"]
mod vanishing_roles;

#[path = "quotient_roles.rs"]
mod quotient_roles;

#[path = "key_roles.rs"]
mod key_roles;
