//! Live-cell cleanup before private composition material reaches its final owner.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;

fn private(index: u64) -> E {
    E::from_coefficients([F(index), F(index + 1), F(index + 2), F(index + 3)]).unwrap()
}
fn chunks() -> Vec<Vec<Vec<E>>> {
    vec![
        (0..COMPOSITION_DEGREE_CHUNKS)
            .map(|index| vec![private(index as u64 + 3); index + 1])
            .collect(),
    ]
}

#[test]
fn composition_precursors_clear_populated_lanes_on_success_error_and_unwind() {
    for outcome in 0..3 {
        let (result, records) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| -> Result<(), ZkX509StarkErrorV1> {
                let mut owner =
                    ZeroizingExtensionLanesV1::new(Vec::new(), zeroize_extension_lanes_v1);
                owner.push(vec![vec![private(7); 3]]);
                owner.push(vec![vec![private(11); 5], vec![private(13); 7]]);
                match outcome {
                    0 => Ok(()),
                    1 => Err(ZkX509StarkErrorV1::ProfileMismatch),
                    _ => panic!("synthetic composition construction unwind"),
                }
            })
        });
        assert_eq!(result.is_err(), outcome == 2);
        if let Ok(inner) = result {
            assert_eq!(inner.is_err(), outcome == 1);
        }
        assert_eq!(records.iter().map(|record| record.cells).sum::<usize>(), 15);
        assert!(
            records
                .iter()
                .all(|record| record.nonzero_before == record.cells)
        );
        assert!(records.iter().all(|record| record.nonzero_after == 0));
    }
}

#[test]
fn composition_growth_clears_displaced_allocation_and_reserved_additions_keep_addresses() {
    let mut accumulator = ZeroizingExtensionLanesV1::new(chunks(), zeroize_extension_lanes_v1);
    let contribution = vec![vec![vec![private(19); 13]; COMPOSITION_DEGREE_CHUNKS]];
    let before = accumulator.to_vec();
    let (result, records) = inspection::observe_v1(|| {
        add_main_composition_coefficient_chunks_v1(&mut accumulator, &contribution, 32)
    });
    result.unwrap();
    assert!(records.iter().any(|record| record.nonzero_before > 0));
    assert!(records.iter().all(|record| record.nonzero_after == 0));
    for (column, old) in accumulator[0].iter().zip(&before[0]) {
        for (index, actual) in column.iter().enumerate() {
            assert_eq!(
                *actual,
                old.get(index).copied().unwrap_or(E::ZERO).add(private(19))
            );
        }
    }
    let addresses = accumulator[0].iter().map(Vec::as_ptr).collect::<Vec<_>>();
    add_main_composition_coefficient_chunks_v1(&mut accumulator, &contribution, 32).unwrap();
    assert_eq!(
        addresses,
        accumulator[0].iter().map(Vec::as_ptr).collect::<Vec<_>>()
    );
    let before = accumulator.to_vec();
    let mut malformed = contribution.clone();
    malformed[0][COMPOSITION_DEGREE_CHUNKS - 1].resize(33, private(23));
    assert!(add_main_composition_coefficient_chunks_v1(&mut accumulator, &malformed, 32).is_err());
    assert_eq!(&*accumulator, &before);
}

#[test]
fn composition_cancellation_retains_extent_and_clears_on_drop() {
    let mut accumulator = ZeroizingExtensionLanesV1::new(chunks(), zeroize_extension_lanes_v1);
    let mut contribution = chunks();
    for chunk in &mut contribution[0] {
        for coefficient in chunk {
            *coefficient = E::ZERO.sub(*coefficient);
        }
    }
    let extents = accumulator[0].iter().map(Vec::len).collect::<Vec<_>>();
    let initialized = extents.iter().sum::<usize>();
    let (result, records) = inspection::observe_v1(|| {
        let result =
            add_main_composition_coefficient_chunks_v1(&mut accumulator, &contribution, 32);
        assert_eq!(
            accumulator[0].iter().map(Vec::len).collect::<Vec<_>>(),
            extents
        );
        assert!(
            accumulator[0]
                .iter()
                .flatten()
                .all(|value| *value == E::ZERO)
        );
        // The original clearing owner must visit the full public extent even
        // though exact cancellation has already made every coefficient zero.
        drop(accumulator);
        result
    });
    result.unwrap();
    assert_eq!(
        records.iter().map(|record| record.cells).sum::<usize>(),
        initialized
    );
    assert!(records.iter().all(|record| record.nonzero_after == 0));
}

#[test]
fn composition_evaluation_clears_prior_results_on_late_error_and_matches_horner() {
    let mut source = chunks();
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let output = ZeroizingExtensionLanesV1::new(
        evaluate_main_composition_columns_v1(&source, 32, root, 16).unwrap(),
        zeroize_extension_lanes_v1,
    );
    for (coefficients, evaluated) in source[0].iter().zip(&output[0]) {
        for (row, actual) in evaluated.iter().enumerate() {
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
            let expected = coefficients.iter().rev().fold(E::ZERO, |sum, coefficient| {
                sum.mul_base(x).add(*coefficient)
            });
            assert_eq!(*actual, expected);
        }
    }
    source[0][2].resize(17, private(29));
    let (result, records) =
        inspection::observe_v1(|| evaluate_main_composition_columns_v1(&source, 32, root, 16));
    assert!(result.is_err());
    assert_eq!(records.iter().map(|record| record.cells).sum::<usize>(), 64);
    assert!(records.iter().any(|record| record.nonzero_before > 0));
    assert!(records.iter().all(|record| record.nonzero_after == 0));
    assert!(goldilocks_fp4_evaluate_coset_v1(&[private(31)], 32, F::ZERO, F(7)).is_err());
}

#[test]
fn quotient_row_reuse_matches_full_gather_at_task_boundaries_wrap_and_other_strides() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let rows = 17;
    for registration in layout.registered_segments {
        let matrix = |width: usize, salt: usize| {
            (0..width)
                .map(|column| {
                    (0..rows)
                        .map(|row| F((salt + 29 * column + 7 * row) as u64))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let base = matrix(registration.segment.base_width, 3);
        let aux = matrix(registration.segment.aux_width, 13);
        let fixed = matrix(registration.segment.fixed_width, 23);
        let mut expected = MainQuotientRowScratchV1::new_v1(registration).unwrap();
        let mut actual = MainQuotientRowScratchV1::new_v1(registration).unwrap();
        let allocations = |scratch: &MainQuotientRowScratchV1| {
            let mut owners = [
                &scratch.base_current,
                &scratch.base_next,
                &scratch.aux_current,
                &scratch.aux_next,
                &scratch.fixed_current,
                &scratch.fixed_next,
            ]
            .map(|column| (column.as_ptr(), column.0.capacity()));
            owners.sort_unstable();
            owners
        };
        let original_owners = allocations(&actual);
        for stride in [1, 2, 8] {
            for task_start in [0, 3, rows - 1] {
                for offset in 0..rows + 2 {
                    let row = (task_start + offset) % rows;
                    let next = (row + stride) % rows;
                    expected.fill_v1(row, next, &base, &aux, &fixed);
                    actual.fill_or_advance_v1(
                        row,
                        next,
                        stride == 1 && offset != 0,
                        &base,
                        &aux,
                        &fixed,
                    );
                    for (actual, expected) in [
                        (&actual.base_current, &expected.base_current),
                        (&actual.base_next, &expected.base_next),
                        (&actual.aux_current, &expected.aux_current),
                        (&actual.aux_next, &expected.aux_next),
                        (&actual.fixed_current, &expected.fixed_current),
                        (&actual.fixed_next, &expected.fixed_next),
                    ] {
                        assert_eq!(&**actual, &**expected);
                    }
                    assert_eq!(allocations(&actual), original_owners);
                }
            }
        }
    }
}

#[test]
fn reusable_residue_owner_clears_hidden_capacity_on_success_error_and_unwind() {
    assert_eq!(
        MainResidueScratchV1::new_v1(usize::MAX).err(),
        Some(ZkX509StarkErrorV1::AllocationFailure)
    );
    for outcome in 0..3 {
        let (result, records) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| -> Result<(), ZkX509StarkErrorV1> {
                let mut owner = MainResidueScratchV1::new_v1(890)?;
                let allocation = (owner.values.as_ptr(), owner.values.capacity());
                owner.values.resize(890, F(47));
                owner.values.clear();
                owner.values.extend([F(53); 3]);
                assert_eq!((owner.values.as_ptr(), owner.values.capacity()), allocation);
                match outcome {
                    0 => Ok(()),
                    1 => Err(ZkX509StarkErrorV1::ProfileMismatch),
                    _ => panic!("synthetic residue evaluator unwind"),
                }
            })
        });
        assert_eq!(result.is_err(), outcome == 2);
        if let Ok(result) = result {
            assert_eq!(result.is_err(), outcome == 1);
        }
        assert_eq!(
            records.iter().map(|record| record.cells).sum::<usize>(),
            890
        );
        assert!(records.iter().all(|record| record.nonzero_after == 0));
    }
}

#[test]
fn reusable_residue_scratch_is_bounded_by_public_tasks_within_original_runtime_reserve() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    for registration in layout.registered_segments.iter().copied() {
        let scratch = MainQuotientRowScratchV1::new_v1(registration).unwrap();
        let expected = if matches!(
            registration.segment.adapter,
            SegmentAdapterIdV1::StrictDer | SegmentAdapterIdV1::Sha256CallBus
        ) {
            registration.segment.constraint_count
        } else {
            0
        };
        assert_eq!(scratch.residues.values.capacity(), expected);
        let per_task =
            expected * core::mem::size_of::<F>() + core::mem::size_of::<MainResidueScratchV1>();
        assert!(128 * per_task <= main_resources::MAIN_COMPOSITION_RESIDUE_SCRATCH_BYTES_V1);
    }
    assert_eq!(
        main_resources::MAIN_COMPOSITION_RESIDUE_SCRATCH_BYTES_V1,
        2_199_552
    );
    assert_eq!(
        main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1,
        1 << 30
    );
    use super::super::super::sha_word_stark::wide_fold::{ShaWordFoldPowersV1, ShaWordWideFoldV1};
    assert_eq!(
        main_resources::MAIN_COMPOSITION_SHA_FOLD_BYTES_V1,
        ShaWordFoldPowersV1::HEAP_PAYLOAD_BYTES_V1
            + ShaWordFoldPowersV1::CONSTRUCTION_SCRATCH_BYTES_V1
            + main_resources::MAIN_COMPOSITION_PROVIDER_STORAGE_BYTES_V1
            + core::mem::size_of::<Option<MainLog19CompositionRegistrationV1<'_, '_, '_>>>()
            + 128 * core::mem::size_of::<ShaWordWideFoldV1<'_>>()
    );
    assert_eq!(
        main_resources::MAIN_COMPOSITION_PROVIDER_STORAGE_BYTES_V1,
        2 * core::mem::size_of::<[MainProverConstraintProviderV1<'_, '_>; 6]>()
            + core::mem::size_of::<MainProverConstraintProviderV1<'_, '_>>()
            + core::mem::size_of::<Vec<MainProverConstraintProviderV1<'_, '_>>>()
    );
    assert_eq!(FULL_PROFILE_TRACE_GROUPS_V1, 6);
    assert_eq!(ShaWordFoldPowersV1::HEAP_PAYLOAD_BYTES_V1, 14_112);
    assert_eq!(
        core::mem::size_of::<ShaWordFoldPowersV1>(),
        core::mem::size_of::<usize>()
    );
    assert!(
        main_resources::MAIN_COMPOSITION_RESIDUE_SCRATCH_BYTES_V1
            + main_resources::MAIN_COMPOSITION_SHA_FOLD_BYTES_V1
            < main_resources::MAIN_PROVER_RUNTIME_RESERVE_BYTES_V1
    );
}
