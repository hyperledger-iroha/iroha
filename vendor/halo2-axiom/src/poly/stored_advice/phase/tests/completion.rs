//! Terminal receipt ordering, bounded access, and complete-owner destruction regressions.
//!
//! These use the parent module's actual IPA commitments and seeded transcript fixtures. The
//! recording backend retains plaintext for assertions; drop counts do not qualify a real
//! spool's key/file cleanup, caller-created secret copies, or production resource limits.

use super::*;

fn reset_blind_drops() {
    BLIND_DROPS.with(|counts| counts.set((0, 0)));
}

fn assert_dropped(backend: &Rc<Backend>, count: usize) {
    assert_eq!(backend.record.borrow().snapshot_drops, count);
    BLIND_DROPS.with(|counts| assert_eq!(counts.get(), (count, 0)));
    assert!(!backend.busy.get());
}

fn interleaved<F: Field>() -> ConstraintSystem<F> {
    let mut meta = ConstraintSystem::default();
    meta.advice_column(); // Global column 0, phase 0.
    meta.advice_column_in(SecondPhase); // Global column 1, phase 1.
    meta.challenge_usable_after(SecondPhase); // Global challenge 0, phase 1.
    meta.advice_column(); // Global column 2, phase 0.
    meta.challenge_usable_after(FirstPhase); // Global challenge 1, phase 0.
    meta.advice_column_in(ThirdPhase); // Global column 3, phase 2.
    meta.challenge_usable_after(ThirdPhase); // Global challenge 2, phase 2.
    meta.advice_column_in(SecondPhase); // Global column 4, phase 1.
    meta.challenge_usable_after(FirstPhase); // Global challenge 3, phase 0.
    meta
}

fn absorbed_through<'params, C>(
    params: &'params ParamsIPA<C>,
    backend: &Rc<Backend>,
    last_phase: usize,
) -> (
    CommittedStoredPhaseV1<'params, C, Snapshot>,
    CountingRng,
    CountingTranscript<C>,
)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let domain = EvaluationDomain::<C::Scalar>::new(3, params.k());
    let plan = admit_stored_phase_plan_v1(params, &domain, &interleaved()).unwrap();
    let mut phase_writers: Vec<_> = (0..3)
        .map(|phase| writers(&plan, phase, 11 + phase as u64 * 13, backend))
        .collect();
    let first = std::mem::take(&mut phase_writers[0]);
    let mut owner = StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, first).unwrap();
    let mut rng = CountingRng::new(backend);
    let mut transcript = CountingTranscript::<C>::new();
    transcript.common_scalar(C::Scalar::from(101)).unwrap();
    for phase in 0..=last_phase {
        let active = owner.active.as_ref().unwrap();
        let usable = active.session.plan.usable_rows;
        let columns = active.session.plan.phases[phase].columns.clone();
        let prior = active.session.challenges[1].unwrap_or(C::Scalar::ZERO);
        for column in columns {
            for (row, value) in phase_inputs(column, usable, prior) {
                owner.assign_discarding_value(column, row, value).unwrap();
            }
        }
        let committed = owner
            .finish(&mut rng)
            .unwrap()
            .absorb(&mut transcript)
            .unwrap();
        if phase == last_phase {
            return (committed, rng, transcript);
        }
        owner = committed
            .begin_next(std::mem::take(&mut phase_writers[phase + 1]))
            .unwrap();
    }
    unreachable!("fixture always includes phase zero")
}

fn completed_oracle<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    // k=9 crosses the backend's 256-scalar chunk boundary; k=4 has a short final chunk.
    for k in [4, 9] {
        reset_blind_drops();
        let params = ParamsIPA::<C>::new(k);
        let backend = Rc::new(Backend::default());
        let (committed, mut rng, transcript) = absorbed_through(&params, &backend, 2);
        assert!(committed.is_complete());
        assert_eq!(
            committed
                .session
                .columns
                .iter()
                .map(|column| column.layout.column())
                .collect::<Vec<_>>(),
            vec![0, 2, 1, 4, 3]
        );
        let capacity = committed.session.columns.capacity();
        let allocation = committed.session.columns.as_ptr();
        let greatest_ordinal = committed.session.greatest_ordinal;
        let mut expected: Vec<_> = committed
            .session
            .columns
            .iter()
            .map(|column| {
                (
                    column.layout,
                    column.snapshot.values.as_ptr(),
                    column.snapshot.values.clone(),
                    column.blind.0.0,
                )
            })
            .collect();
        expected.sort_unstable_by_key(|column| column.0.column());
        let expected_challenges: Vec<_> = (0..4)
            .map(|index| committed.challenge(index).unwrap())
            .collect();
        let (draws, reads) = {
            let record = backend.record.borrow();
            (record.rng_draws, record.read_count)
        };
        let writes = transcript.writes;
        let squeezes = transcript.squeezes;
        let mut oracle_rng = rng.inner.clone();
        let mut complete = committed.into_complete().unwrap();
        assert!(std::ptr::eq(complete.params().unwrap(), &params));
        assert_eq!(complete.proof_context().unwrap(), Some([9; 32]));
        assert_eq!(
            complete.challenges().unwrap().collect::<Vec<_>>(),
            expected_challenges
        );
        assert_eq!(complete.layouts().unwrap().len(), 5);
        let layouts: Vec<_> = complete.layouts().unwrap().collect();
        assert_eq!(
            layouts,
            expected.iter().map(|entry| entry.0).collect::<Vec<_>>()
        );
        let session = complete.session.as_ref().unwrap();
        assert_eq!(session.columns.capacity(), capacity);
        assert_eq!(session.columns.as_ptr(), allocation);
        assert_eq!(session.greatest_ordinal, greatest_ordinal);
        for (column, expected) in session.columns.iter().zip(&expected) {
            assert_eq!(column.snapshot.values.as_ptr(), expected.1);
        }
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_eq!(backend.record.borrow().rng_draws, draws);
        BLIND_DROPS.with(|counts| assert_eq!(counts.get(), (0, 0)));
        // Repeat a complete reverse-order traversal: these are the original repeatable
        // snapshots, not consumed iterators or re-created witnesses. with_column would panic.
        let mut callback_reads = 0;
        for _ in 0..2 {
            for (layout, _, encoded, blind) in expected.iter().rev() {
                for chunk in (0..layout.chunk_count() as u64).rev() {
                    let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
                    let count = layout.chunk_scalar_count(chunk).unwrap();
                    let returned = complete
                        .with_chunk(*layout, chunk, |values, actual_blind| {
                            assert_eq!(values, &encoded[start..start + count]);
                            assert_eq!(actual_blind.0, *blind);
                            assert!(values.len() <= STORED_SCALARS_PER_CHUNK_V1);
                            callback_reads += 1;
                            Ok(values.len())
                        })
                        .unwrap();
                    assert_eq!(returned, count);
                    assert!(!backend.busy.get());
                }
            }
        }
        assert_eq!(backend.record.borrow().read_count, reads + callback_reads);
        assert_eq!(backend.record.borrow().rng_draws, draws);
        assert_eq!(transcript.writes, writes);
        assert_eq!(transcript.squeezes, squeezes);
        let mut actual_next = [0; 64];
        let mut expected_next = [0; 64];
        rng.fill_bytes(&mut actual_next);
        oracle_rng.fill_bytes(&mut expected_next);
        assert_eq!(actual_next, expected_next);
        drop(complete);
        assert_dropped(&backend, 5);
    }
}

#[test]
fn eq_fp_completion_preserves_interleaved_global_receipts_and_bounded_reads() {
    completed_oracle::<EqAffine>();
}

#[test]
fn ep_fq_completion_preserves_interleaved_global_receipts_and_bounded_reads() {
    completed_oracle::<EpAffine>();
}

fn empty_completion<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    reset_blind_drops();
    let params = ParamsIPA::<C>::new(4);
    let domain = EvaluationDomain::<C::Scalar>::new(3, 4);
    let plan = admit_stored_phase_plan_v1(&params, &domain, &ConstraintSystem::default()).unwrap();
    let backend = Rc::new(Backend::default());
    let mut rng = CountingRng::new(&backend);
    let mut transcript = CountingTranscript::<C>::new();
    let owner = StoredPhaseAssignmentsV1::<C, Writer>::begin(plan, vec![]).unwrap();
    let complete = owner
        .finish(&mut rng)
        .unwrap()
        .absorb(&mut transcript)
        .unwrap()
        .into_complete()
        .unwrap();
    assert!(std::ptr::eq(complete.params().unwrap(), &params));
    assert_eq!(complete.proof_context().unwrap(), None);
    assert_eq!(complete.layouts().unwrap().len(), 0);
    assert_eq!(complete.challenges().unwrap().count(), 0);
    assert_eq!(backend.record.borrow().rng_draws, 0);
    assert_eq!(backend.record.borrow().read_count, 0);
    assert_eq!(transcript.writes, 0);
    assert_eq!(transcript.squeezes, 0);
    drop(complete);
    assert_dropped(&backend, 0);
}

#[test]
fn both_fields_empty_completion_preserves_absence_of_context_and_side_effects() {
    empty_completion::<EqAffine>();
    empty_completion::<EpAffine>();
}

fn incomplete_completion<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    for (last_phase, expected_drops) in [(0, 2), (1, 4)] {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, transcript) = absorbed_through(&params, &backend, last_phase);
        assert!(!committed.is_complete());
        let (reads, draws) = {
            let record = backend.record.borrow();
            (record.read_count, record.rng_draws)
        };
        let (writes, squeezes) = (transcript.writes, transcript.squeezes);
        assert!(matches!(
            committed.into_complete(),
            Err(StoredPhaseErrorV1::Admission)
        ));
        assert_dropped(&backend, expected_drops);
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_eq!(backend.record.borrow().rng_draws, draws);
        assert_eq!((transcript.writes, transcript.squeezes), (writes, squeezes));
    }
}

#[test]
fn both_fields_incomplete_consumption_destroys_every_absorbed_phase() {
    incomplete_completion::<EqAffine>();
    incomplete_completion::<EpAffine>();
}

#[test]
fn completion_rejects_inconsistent_terminal_receipts_and_destroys_all_secrets() {
    let params = ParamsIPA::<EqAffine>::new(4);
    // Internal fault injection checks the terminal defense; public callers cannot mutate
    // the admitted plan or receipt fields. The snapshots already contain valid commitments.
    for fault in 0..15 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (mut committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let session = &mut committed.session;
        match fault {
            0 => session.next_phase += 1,
            1 => session.challenges[1] = None,
            2 => {
                session.challenges.pop();
            }
            3 => {
                session.columns.pop();
            }
            4 => session.columns.swap(0, 1),
            5 => session.proof_context = Some([8; 32]),
            6 => session.proof_context = None,
            7 => session.greatest_ordinal = Some(99),
            8 => session.columns[0].snapshot.layout.ordinal += 1,
            9 => session.columns[0].layout.proof_context = [0; 32],
            10 => session.columns[0].layout.column = 1,
            11 => session.columns[0].layout.phase = 1,
            12 => session.columns[0].layout.k = 5,
            13 => {
                // Substituting both cached and live identities still cannot repeat an ordinal.
                session.columns[1].layout.ordinal = session.columns[0].layout.ordinal;
                session.columns[1].snapshot.layout = session.columns[1].layout;
            }
            14 => {
                // Keep phase-major identities self-consistent but duplicate a global column.
                session.plan.phases[0].columns[1] = 0;
                session.columns[1].layout.column = 0;
                session.columns[1].snapshot.layout = session.columns[1].layout;
            }
            _ => unreachable!(),
        }
        let reads = backend.record.borrow().read_count;
        assert!(matches!(
            committed.into_complete(),
            Err(StoredPhaseErrorV1::Admission)
        ));
        assert_dropped(&backend, 5);
        assert_eq!(backend.record.borrow().read_count, reads);
    }
}

fn assert_poisoned(
    complete: &mut CompleteStoredAdviceV1<'_, EqAffine, Snapshot>,
    layout: StoredAdviceLayoutV1,
) {
    assert!(matches!(
        complete.params(),
        Err(StoredPhaseErrorV1::Poisoned)
    ));
    assert!(matches!(
        complete.proof_context(),
        Err(StoredPhaseErrorV1::Poisoned)
    ));
    assert!(matches!(
        complete.layouts(),
        Err(StoredPhaseErrorV1::Poisoned)
    ));
    assert!(matches!(
        complete.challenges(),
        Err(StoredPhaseErrorV1::Poisoned)
    ));
    let mut called = false;
    assert_eq!(
        complete.with_chunk(layout, 0, |_, _| {
            called = true;
            Ok(())
        }),
        Err(StoredPhaseErrorV1::Poisoned)
    );
    assert!(!called);
}

#[test]
fn completed_invalid_requests_destroy_all_receipts_before_backend_or_consumer_access() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for fault in 0..8 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        let original = complete.layouts().unwrap().next().unwrap();
        let mut expected = original;
        let mut chunk = 0;
        let error = match fault {
            0 => {
                expected.proof_context = [8; 32];
                StoredAdviceErrorV1::Context
            }
            1 => {
                expected.ordinal += 1;
                StoredAdviceErrorV1::Context
            }
            2 => {
                expected.phase = 1;
                StoredAdviceErrorV1::Context
            }
            3 => {
                expected.k += 1;
                StoredAdviceErrorV1::Context
            }
            4 => {
                expected.column = 4;
                StoredAdviceErrorV1::Context
            }
            5 => {
                chunk = u64::MAX;
                StoredAdviceErrorV1::ChunkIndex
            }
            6 => {
                complete.session.as_mut().unwrap().columns[0]
                    .snapshot
                    .layout
                    .ordinal += 1;
                StoredAdviceErrorV1::Context
            }
            7 => {
                expected.column = u32::MAX;
                StoredAdviceErrorV1::Context
            }
            _ => unreachable!(),
        };
        let reads = backend.record.borrow().read_count;
        let mut called = false;
        let actual = complete.with_chunk(expected, chunk, |_, _| {
            called = true;
            Ok(())
        });
        let expected_error = if fault == 7 {
            StoredPhaseErrorV1::Admission
        } else {
            error.into()
        };
        assert_eq!(actual, Err(expected_error));
        assert!(!called);
        assert_eq!(backend.record.borrow().read_count, reads);
        assert_dropped(&backend, 5);
        assert_poisoned(&mut complete, original);
    }
}

#[test]
fn completed_storage_encoding_and_identity_failures_poison_the_entire_owner() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for fault in 0..4 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        let layout = complete.layouts().unwrap().nth(3).unwrap();
        let location = Some((layout.column(), 0));
        let expected = {
            let mut record = backend.record.borrow_mut();
            match fault {
                0 => {
                    record.fail_read = location;
                    StoredAdviceErrorV1::Authentication
                }
                1 => {
                    record.corrupt_read = location;
                    StoredAdviceErrorV1::Encoding
                }
                2 => {
                    record.short_read = location;
                    StoredAdviceErrorV1::Encoding
                }
                3 => {
                    record.change_after_read = location;
                    StoredAdviceErrorV1::Context
                }
                _ => unreachable!(),
            }
        };
        let reads = backend.record.borrow().read_count;
        let mut called = false;
        assert_eq!(
            complete.with_chunk(layout, 0, |_, _| {
                called = true;
                Ok(())
            }),
            Err(expected.into())
        );
        assert_eq!(called, fault == 3);
        assert_eq!(backend.record.borrow().read_count, reads + 1);
        assert_dropped(&backend, 5);
        assert_poisoned(&mut complete, layout);
    }
}

#[test]
fn completed_consumer_error_and_backend_or_consumer_unwind_drop_every_receipt() {
    let params = ParamsIPA::<EqAffine>::new(4);
    for fault in 0..3 {
        reset_blind_drops();
        let backend = Rc::new(Backend::default());
        let (committed, _rng, _transcript) = absorbed_through(&params, &backend, 2);
        let mut complete = committed.into_complete().unwrap();
        let layout = complete.layouts().unwrap().nth(1).unwrap();
        if fault == 1 {
            backend.record.borrow_mut().panic_read = Some((layout.column(), 0));
        }
        let mut called = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            complete.with_chunk(layout, 0, |_, _| {
                called = true;
                assert_ne!(fault, 2, "injected completed consumer unwind");
                Err::<(), _>(StoredAdviceErrorV1::Consumer)
            })
        }));
        if fault == 0 {
            assert_eq!(result.unwrap(), Err(StoredAdviceErrorV1::Consumer.into()));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(called, fault != 1);
        assert_dropped(&backend, 5);
        assert_poisoned(&mut complete, layout);
    }
}
