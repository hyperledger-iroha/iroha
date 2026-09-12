//! Actual-spool adapter tests; no mock cipher, digest authority, or proof qualification.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ff::WithSmallOrderMulGroup;
use halo2_proofs::{
    halo2curves::pasta::{Fp, Fq},
    plonk::Assigned,
    poly::{
        EvaluationDomain,
        stored_advice::{
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
    provider: &mut CoreStoredAdviceProviderV1,
    field: StoredPastaFieldV1,
    k: u32,
) -> CoreStoredAdviceSnapshotV1 {
    let mut writer = provider
        .create(field, StoredPolynomialBasisV1::Lagrange, k, 3, 0)
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

#[test]
fn both_pasta_fields_roundtrip_out_of_order_and_materialize_exactly_one_column() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    let mut writer = provider
        .create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            9,
            1,
            0,
        )
        .unwrap();
    let valid = vec![scalar(1); 256];
    assert_eq!(
        writer.write_chunk(1, &valid),
        Err(StoredAdviceErrorV1::WriteOrder)
    );
    assert_eq!(
        writer.write_chunk(0, &valid[..255]),
        Err(StoredAdviceErrorV1::WriteOrder)
    );
    let invalid = vec![[0xff; 32]; 256];
    assert_eq!(
        writer.write_chunk(0, &invalid),
        Err(StoredAdviceErrorV1::Encoding)
    );
    writer.write_chunk(0, &valid).unwrap();
    assert_eq!(
        writer.write_chunk(0, &valid),
        Err(StoredAdviceErrorV1::WriteOrder)
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
            1,
            0,
        )
        .unwrap();
    assert!(matches!(
        incomplete.seal(),
        Err(StoredAdviceErrorV1::Incomplete)
    ));
}

#[test]
fn expected_metadata_and_slot_mismatches_are_retryable_and_never_expose_plaintext() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
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
            StoredAdviceLayoutV1::new(
                provider.proof_context,
                original.ordinal(),
                field,
                basis,
                k,
                column,
                phase,
            )
            .unwrap(),
        );
    }
    wrong.push(
        StoredAdviceLayoutV1::new(
            provider.proof_context,
            original.ordinal() + 1,
            original.field(),
            original.basis(),
            original.k(),
            original.column(),
            original.phase(),
        )
        .unwrap(),
    );
    wrong.push(
        StoredAdviceLayoutV1::new(
            [9; 32],
            original.ordinal(),
            original.field(),
            original.basis(),
            original.k(),
            original.column(),
            original.phase(),
        )
        .unwrap(),
    );
    for expected in wrong {
        assert_eq!(
            snapshot.with_column(expected, |_| panic!("wrong metadata exposed plaintext")),
            Err::<(), _>(StoredAdviceErrorV1::Context)
        );
        assert_eq!(
            snapshot.with_chunk(expected, 0, |_| panic!("wrong metadata exposed plaintext")),
            Err::<(), _>(StoredAdviceErrorV1::Context)
        );
    }
    assert_eq!(
        snapshot.with_chunk(original, 1, |_| panic!("invalid slot exposed plaintext")),
        Err::<(), _>(StoredAdviceErrorV1::ChunkIndex)
    );
    snapshot.with_column(original, |_| Ok(())).unwrap();
}

#[test]
fn shared_window_blocks_nested_materializations_and_recovers_after_release() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    let mut first = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let mut second = filled(&mut provider, StoredPastaFieldV1::Fq, 1);
    let second_layout = second.layout();
    first
        .with_column(first.layout(), |_| {
            assert_eq!(
                second.with_column(second_layout, |_| panic!("two columns materialized")),
                Err::<(), _>(StoredAdviceErrorV1::Busy)
            );
            assert_eq!(
                second.with_chunk(second_layout, 0, |_| panic!("nested chunk materialized")),
                Err::<(), _>(StoredAdviceErrorV1::Busy)
            );
            assert!(matches!(
                provider.create(
                    StoredPastaFieldV1::Fp,
                    StoredPolynomialBasisV1::Lagrange,
                    1,
                    0,
                    0
                ),
                Err(StoredAdviceErrorV1::Busy)
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    for full_column in [false, true] {
        for panic in [false, true] {
            let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
            let layout = snapshot.layout();
            let result = catch_unwind(AssertUnwindSafe(|| {
                let consume = |_: &[[u8; 32]]| -> Result<(), StoredAdviceErrorV1> {
                    if panic {
                        panic!("consumer panic");
                    }
                    Err(StoredAdviceErrorV1::Consumer)
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
                assert_eq!(result.unwrap(), Err(StoredAdviceErrorV1::Consumer));
            }
            assert!(snapshot.raw.is_none());
            assert!(!provider.window.get());
            assert_eq!(
                snapshot.with_chunk(layout, 0, |_| Ok(())),
                Err(StoredAdviceErrorV1::Poisoned)
            );
            let mut sibling = filled(&mut provider, StoredPastaFieldV1::Fq, 1);
            sibling.with_column(sibling.layout(), |_| Ok(())).unwrap();
        }
    }
}

#[test]
fn injected_operational_failures_and_unwind_leave_no_live_read_owner() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    for error in [
        StoredAdviceErrorV1::Allocation,
        StoredAdviceErrorV1::Storage,
        StoredAdviceErrorV1::Authentication,
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
            Err(StoredAdviceErrorV1::Poisoned)
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    for malformed_padding in [false, true] {
        // Test-only construction bypasses the adapter writer to supply an authenticated
        // malformed record. No raw-backend constructor is exposed by the production adapter.
        let writer = provider
            .create(
                StoredPastaFieldV1::Fp,
                StoredPolynomialBasisV1::Lagrange,
                1,
                0,
                0,
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
        let mut snapshot = CoreStoredAdviceSnapshotV1 {
            layout,
            raw: Some(raw.seal_v1().unwrap()),
            _lease: writer.lease,
            window: Rc::clone(&provider.window),
            injected_read_error: None,
            panic_on_read: false,
        };
        assert_eq!(
            snapshot.with_column(layout, |_| panic!("malformed bytes exposed")),
            Err::<(), _>(StoredAdviceErrorV1::Encoding)
        );
        assert!(snapshot.raw.is_none());
        assert!(!provider.window.get());
    }
}

#[test]
fn provider_contexts_are_fresh_ordinals_monotonic_and_handle_count_bounded() {
    let directory = tempfile::tempdir().unwrap();
    let mut first = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    let second = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
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
            0,
            0
        ),
        Err(StoredAdviceErrorV1::Capacity)
    ));
}

#[test]
fn released_snapshots_allow_more_than_the_live_limit_without_reusing_identity() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    // Exercise the production admission logic with a small quota, without requiring
    // more descriptors than the host's process limit.
    provider.handles = Rc::new(LiveSnapshotBudget {
        live: Cell::new(0),
        limit: 2,
    });
    let create = |provider: &mut CoreStoredAdviceProviderV1| {
        provider.create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            0,
            0,
            0,
        )
    };
    let mut writer = create(&mut provider).unwrap();
    let other = create(&mut provider).unwrap();
    let ordinal = provider.next_ordinal;
    assert!(matches!(
        create(&mut provider),
        Err(StoredAdviceErrorV1::Capacity)
    ));
    assert_eq!(provider.next_ordinal, ordinal);
    writer.write_chunk(0, &[scalar(3)]).unwrap();
    let snapshot = writer.seal().unwrap();
    assert_eq!(provider.handles.live.get(), 2, "seal moves the lease");
    assert!(matches!(
        create(&mut provider),
        Err(StoredAdviceErrorV1::Capacity)
    ));
    drop(other);
    assert_eq!(provider.handles.live.get(), 1);
    let incomplete = create(&mut provider).unwrap();
    assert!(matches!(
        incomplete.seal(),
        Err(StoredAdviceErrorV1::Incomplete)
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    let mut snapshot = filled(&mut provider, StoredPastaFieldV1::Fp, 1);
    let original = snapshot.layout();
    // A test-only metadata substitution bypasses the public expected-layout preflight.
    // The crypto spool must still reject its distinct authenticated context.
    snapshot.layout = StoredAdviceLayoutV1::new(
        provider.proof_context,
        original.ordinal(),
        StoredPastaFieldV1::Fq,
        original.basis(),
        original.k(),
        original.column(),
        original.phase(),
    )
    .unwrap();
    assert_eq!(
        snapshot.with_column(snapshot.layout(), |_| panic!("substituted field exposed")),
        Err::<(), _>(StoredAdviceErrorV1::Context)
    );
    assert!(snapshot.raw.is_none());

    let writer = provider
        .create(
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            9,
            0,
            0,
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
    let mut snapshot = CoreStoredAdviceSnapshotV1 {
        layout,
        raw: Some(raw.seal_v1().unwrap()),
        _lease: writer.lease,
        window: Rc::clone(&provider.window),
        injected_read_error: None,
        panic_on_read: false,
    };
    assert_eq!(
        snapshot.with_column(layout, |_| panic!("partially decoded column exposed")),
        Err::<(), _>(StoredAdviceErrorV1::Encoding)
    );
    assert!(snapshot.raw.is_none());
    assert!(!provider.window.get());
}

#[test]
fn spool_errors_have_coarse_nonsecret_failure_classes() {
    for (raw, expected) in [
        (
            ConfidentialSpoolErrorV1::Authentication,
            StoredAdviceErrorV1::Authentication,
        ),
        (
            ConfidentialSpoolErrorV1::Allocation("test allocation"),
            StoredAdviceErrorV1::Allocation,
        ),
        (
            ConfidentialSpoolErrorV1::FileOperation {
                operation: "test read",
                kind: std::io::ErrorKind::UnexpectedEof,
            },
            StoredAdviceErrorV1::Storage,
        ),
        (
            ConfidentialSpoolErrorV1::ContextDigestMismatch,
            StoredAdviceErrorV1::Context,
        ),
        (
            ConfidentialSpoolErrorV1::Poisoned,
            StoredAdviceErrorV1::Poisoned,
        ),
        (
            ConfidentialSpoolErrorV1::EntropyUnavailable,
            StoredAdviceErrorV1::Backend,
        ),
    ] {
        let error = map_spool_error(raw);
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("test "));
    }
}

fn encrypted_assignment_roundtrip<F: StoredAssignmentFieldV1>() {
    let directory = tempfile::tempdir().unwrap();
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    let usable = 1019;
    let mut columns = Vec::new();
    for column in [7, 11] {
        let writer = provider
            .create(
                F::STORED_FIELD,
                StoredPolynomialBasisV1::Lagrange,
                10,
                column,
                2,
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
        Err::<(), _>(StoredAdviceErrorV1::Context)
    );
    for (column, snapshot) in snapshots.iter_mut().enumerate() {
        let layout = snapshot.layout();
        assert_eq!(layout.field(), F::STORED_FIELD);
        assert_eq!(layout.phase(), 2);
        assert_eq!(layout.column(), [7, 11][column]);
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    provider.handles = Rc::new(LiveSnapshotBudget {
        live: Cell::new(0),
        limit: 2,
    });
    let mut predecessor: Option<(CoreStoredAdviceSnapshotV1, [[u8; 32]; 4])> = None;
    for generation in 0..6_u64 {
        let writer = provider
            .create(F::STORED_FIELD, StoredPolynomialBasisV1::Lagrange, 2, 9, 1)
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
                provider.create(F::STORED_FIELD, StoredPolynomialBasisV1::Lagrange, 2, 9, 1),
                Err(StoredAdviceErrorV1::Capacity)
            ));
            let old_layout = old.layout();
            assert_ne!(layout.context_digest(), old_layout.context_digest());
            assert_eq!(
                snapshot.with_chunk(old_layout, 0, |_| panic!(
                    "predecessor identity exposed replacement"
                )),
                Err::<(), _>(StoredAdviceErrorV1::Context)
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
    let mut provider = CoreStoredAdviceProviderV1::new(directory.path()).unwrap();
    // Only source and destination may coexist. Each conversion must release its predecessor
    // before a third authenticated owner can be created, with no lifetime creation quota.
    provider.handles = Rc::new(LiveSnapshotBudget {
        live: Cell::new(0),
        limit: 2,
    });
    let domain = EvaluationDomain::<F>::new(5, 9);
    let writer = provider
        .create(F::STORED_FIELD, Lagrange, 9, 17, 1)
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
        assert_eq!(converted_layout.column(), initial_layout.column());
        assert_eq!(converted_layout.phase(), initial_layout.phase());
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
            Err::<(), _>(StoredAdviceErrorV1::Context)
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
