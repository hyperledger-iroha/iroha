// Isolated complete-source timing helpers; no production implementation surface.

impl ZkX509ShaBatchSegmentBaseSourceV1<'_> {
    pub(crate) fn maintained_fill_v1(
        &self,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        validate_sha_column_batch_extent_v1(
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets,
        )?;
        self.validate_column_request_v1(
            segment,
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets[0],
        )?;
        let mut fills = sha_column_fill_batch_v1(targets)?;
        self.for_each_call_row_v1(|row, original| {
            let base = if let Some((call, call_row)) = original {
                call.validate_fixed_row_shape_v1(call_row)?;
                call.base_row(call_row)?
            } else {
                physical_padding_row_v1(row).base
            };
            for (offset, fill) in fills.iter_mut().enumerate() {
                fill.write_v1(row, base[first_column + offset]);
            }
            Ok(())
        })?;
        finish_sha_column_fill_batch_v1(fills)
    }
}

impl ZkX509ShaBatchSegmentAuxSourceV1<'_> {
    pub(crate) fn maintained_replay_v1(
        &self,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        let binding = self.binding.ok_or(ZkX509ShaCallBusStarkErrorV1::Phase)?;
        validate_sha_segment_binding_families_v1(
            binding.sha_word(),
            binding.sha(),
            binding.rfc5280(),
        )?;
        let source = ZkX509ShaBatchSegmentBaseSourceV1 {
            schedule: self.schedule,
            witnesses: self.witnesses,
            replay: self.replay,
            bound: false,
        };
        source.maintained_fill_v1(segment, first_column, targets)
    }
}

#[derive(Clone, Copy, Debug)]
struct ShaBorrowedTimingCaseV1 {
    segment: usize,
    after_binding: bool,
    first: usize,
    width: usize,
}

fn sha_borrowed_timing_cases_v1() -> Vec<ShaBorrowedTimingCaseV1> {
    let mut cases = Vec::with_capacity(56);
    for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
        for after_binding in [false, true] {
            for (first, width) in [(3, 1), (76, 1), (88, 1), (3, 8), (72, 8), (76, 8), (81, 8)] {
                cases.push(ShaBorrowedTimingCaseV1 {
                    segment,
                    after_binding,
                    first,
                    width,
                });
            }
        }
    }
    cases
}

fn sha_borrowed_timing_outputs_v1(width: usize) -> PrivateTableV1<Vec<F>> {
    PrivateTableV1::new(
        vec![native_column_v1(F::ZERO); width],
        zeroize_field_rows_v1,
    )
}

fn sha_borrowed_timing_operation_v1<const MAINTAINED: bool>(
    case: ShaBorrowedTimingCaseV1,
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    columns: &mut [Vec<F>],
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    let mut source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(schedule, witnesses, case.segment)?;
    let mut targets = columns
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect::<Vec<_>>();
    if case.after_binding {
        let bound = source.bind_v1(credential_binding(0x47))?;
        let result = if MAINTAINED {
            bound.maintained_replay_v1(case.segment, case.first, &mut targets)
        } else {
            bound.replay_base_columns_v1(case.segment, case.first, &mut targets)
        };
        // Genuine bound-owner Drop runs before the caller stops this operation's clock.
        drop(bound);
        result
    } else if MAINTAINED {
        source.maintained_fill_v1(case.segment, case.first, &mut targets)
    } else {
        source.fill_base_columns_v1(case.segment, case.first, &mut targets)
    }
}

fn sha_borrowed_timing_sample_v1<const MAINTAINED: bool>(
    case: ShaBorrowedTimingCaseV1,
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    columns: &mut [Vec<F>],
) -> u128 {
    let started = std::time::Instant::now();
    sha_borrowed_timing_operation_v1::<MAINTAINED>(case, schedule, witnesses, columns).unwrap();
    started.elapsed().as_nanos()
}

fn sha_borrowed_timing_pair_v1(
    case: ShaBorrowedTimingCaseV1,
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    candidate_first: bool,
) -> (u128, u128) {
    let mut maintained = sha_borrowed_timing_outputs_v1(case.width);
    let mut candidate = sha_borrowed_timing_outputs_v1(case.width);
    let (maintained_ns, candidate_ns) = if candidate_first {
        let candidate_ns =
            sha_borrowed_timing_sample_v1::<false>(case, schedule, witnesses, &mut candidate);
        (
            sha_borrowed_timing_sample_v1::<true>(case, schedule, witnesses, &mut maintained),
            candidate_ns,
        )
    } else {
        let maintained_ns =
            sha_borrowed_timing_sample_v1::<true>(case, schedule, witnesses, &mut maintained);
        (
            maintained_ns,
            sha_borrowed_timing_sample_v1::<false>(case, schedule, witnesses, &mut candidate),
        )
    };
    assert!(
        *maintained == *candidate,
        "actual complete-source parity for {case:?}"
    );
    assert!(maintained_ns > 0 && candidate_ns > 0);
    // Destination allocation/parity/clearing are outside source elapsed time.
    // Real private-owner Drop completes before a caller can publish the pair.
    drop(maintained);
    drop(candidate);
    (maintained_ns, candidate_ns)
}

#[test]
fn sha_borrowed_timing_closed_case_inventory_and_owned_worker_context() {
    let cases = sha_borrowed_timing_cases_v1();
    assert_eq!(cases.len(), 56);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    assert_eq!(pool.current_num_threads(), 8);
    pool.install(|| {
        assert_eq!(rayon::current_num_threads(), 8);
        assert!(rayon::current_thread_index().is_some());
        for (index, case) in cases.iter().enumerate() {
            assert!(case.segment < 4 && matches!(case.width, 1 | 8));
            assert!(case.first + case.width <= ZK_X509_SHA_BATCH_BASE_WIDTH_V1);
            assert_eq!(case.segment, index / 14);
            assert_eq!(case.after_binding, index % 14 >= 7);
        }
    });
}

#[test]
fn sha_borrowed_timing_destination_owner_clears_on_actual_unwind() {
    let (result, observations) = super::super::private_table::inspection::observe_v1(|| {
        std::panic::catch_unwind(|| {
            let _columns = PrivateTableV1::new(vec![vec![F(71); 9]; 2], zeroize_field_rows_v1);
            panic!("actual isolated output-owner unwind");
        })
    });
    assert!(result.is_err());
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].cells, 18);
    assert_eq!(observations[0].nonzero_before, 18);
    assert_eq!(observations[0].nonzero_after, 0);
}

#[test]
fn sha_borrowed_timing_complete_source_parity_for_closed_grid() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    pool.install(|| {
        assert_eq!(rayon::current_num_threads(), 8);
        for case in sha_borrowed_timing_cases_v1() {
            let mut maintained = sha_borrowed_timing_outputs_v1(case.width);
            let mut candidate = sha_borrowed_timing_outputs_v1(case.width);
            sha_borrowed_timing_operation_v1::<true>(case, &schedule, &witnesses, &mut maintained)
                .unwrap();
            sha_borrowed_timing_operation_v1::<false>(case, &schedule, &witnesses, &mut candidate)
                .unwrap();
            assert!(
                *maintained == *candidate,
                "untimed closed-grid parity for {case:?}"
            );
        }
    });
}

/// Complete maintained versus borrowed-source observations, separately authorized.
#[test]
#[ignore = "explicit complete-source component timing requires a compiler-idle quiet window"]
fn sha_borrowed_complete_source_timing_v1() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    assert_eq!(pool.current_num_threads(), 8);
    pool.install(|| {
        assert_eq!(rayon::current_num_threads(), 8);
        assert!(rayon::current_thread_index().is_some());
        for (index, case) in sha_borrowed_timing_cases_v1().into_iter().enumerate() {
            let warmup_candidate_first = !index.is_multiple_of(2);
            let (baseline_ns, candidate_ns) = sha_borrowed_timing_pair_v1(case, &schedule, &witnesses, warmup_candidate_first);
            eprintln!("SHA_BORROWED_SOURCE_WARMUP_V1 case={index} disclosed=4 segment={} after_binding={} first={} width={} candidate_first={warmup_candidate_first} baseline_ns={baseline_ns} candidate_ns={candidate_ns} parity=exact workers=8", case.segment, case.after_binding, case.first, case.width);
            for round in 0..4 {
                let candidate_first = !(index + round).is_multiple_of(2);
                let (baseline_ns, candidate_ns) = sha_borrowed_timing_pair_v1(case, &schedule, &witnesses, candidate_first);
                eprintln!("SHA_BORROWED_SOURCE_PAIR_V1 case={index} disclosed=4 segment={} after_binding={} first={} width={} round={round} candidate_first={candidate_first} baseline_ns={baseline_ns} candidate_ns={candidate_ns} parity=exact workers=8", case.segment, case.after_binding, case.first, case.width);
            }
        }
    });
}
